package co.groundglass.cameralink

import android.Manifest
import android.app.Activity
import android.bluetooth.BluetoothAdapter
import android.content.Intent
import android.os.Build
import android.provider.Settings
import android.view.WindowManager
import app.tauri.PermissionState
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import kotlin.concurrent.thread

private const val NEARBY = "nearby"
private const val LOCATION = "location"
private const val LOCAL_NETWORK = "localNetwork"
/** Android 17: the local network needs ACCESS_LOCAL_NETWORK. A literal, as the plugin compiles against 36. */
private const val ANDROID_17 = 37
/** Canon's app kept Bluetooth up and the camera closed it 27 s after its Wi-Fi went off (recorded 2026-10-31). */
private const val HAND_BACK_WAIT_S = 45L

@InvokeArg
class ConnectArgs {
    lateinit var clientId: IntArray
    lateinit var clientName: String
}

@InvokeArg
class KeepAwakeArgs {
    var on: Boolean = false
}

/** A step that can't go on; [sentence] tells the user what to do, [detail] what the camera last said. */
class Stop(val sentence: String, val detail: String? = null) : Exception(sentence)

/** Wakes the paired camera and joins its Wi-Fi. Every outcome resolves: failures are a sentence in the report. */
@TauriPlugin(permissions = [
    Permission(strings = [Manifest.permission.BLUETOOTH_CONNECT, Manifest.permission.BLUETOOTH_SCAN], alias = NEARBY),
    Permission(strings = [Manifest.permission.ACCESS_FINE_LOCATION], alias = LOCATION),
    Permission(strings = ["android.permission.ACCESS_LOCAL_NETWORK"], alias = LOCAL_NETWORK),
])
class CameraLinkPlugin(private val activity: Activity) : Plugin(activity) {
    private val wifi = WifiJoin(activity)
    /** Held while connected: the recordings keep Bluetooth up during live view. */
    private var camera: CameraGatt? = null
    /** The camera finishing up after a release; the next connect waits for it. */
    private var handingBack: Thread? = null

    /** Whether a tap can start now: the screen dims the button when it can't. */
    @Command
    fun ready(invoke: Invoke) {
        invoke.resolve(JSObject().put("ready", blocker() == null))
    }

    @Command
    fun connect(invoke: Invoke) {
        if (getPermissionState(permission()) == PermissionState.GRANTED) {
            turnOn(invoke)
        } else {
            requestPermissionForAlias(permission(), invoke, "permissionAnswered")
        }
    }

    @PermissionCallback
    fun permissionAnswered(invoke: Invoke) {
        if (getPermissionState(permission()) == PermissionState.GRANTED) {
            turnOn(invoke)
        } else {
            invoke.resolve(report().put("failure", "Allow Groundglass in the phone's app settings, then tap again."))
        }
    }

    /**
     * Asks Android to turn on Bluetooth, then Wi-Fi, whichever is off; the connect
     * goes on once both are. Said no, and the report says what to turn on.
     */
    private fun turnOn(invoke: Invoke) {
        when {
            !CameraGatt.isBluetoothOn(activity) ->
                startActivityForResult(invoke, Intent(BluetoothAdapter.ACTION_REQUEST_ENABLE), "bluetoothAnswered")
            // Android 10 and later don't let an app turn Wi-Fi on: its panel lets the user do it.
            wifi.isOff && Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q ->
                startActivityForResult(invoke, Intent(Settings.Panel.ACTION_WIFI), "wifiAnswered")
            else -> start(invoke)
        }
    }

    /** [result] goes unread: whether Bluetooth is on now is what counts. */
    @ActivityCallback
    fun bluetoothAnswered(invoke: Invoke, @Suppress("UNUSED_PARAMETER") result: Any) {
        if (CameraGatt.isBluetoothOn(activity)) turnOn(invoke) else start(invoke)
    }

    @ActivityCallback
    fun wifiAnswered(invoke: Invoke, @Suppress("UNUSED_PARAMETER") result: Any) {
        start(invoke)
    }

    /**
     * Whether the app may reach the camera's address. Before Android 17 it always
     * may; on 17 Android asks once, or not at all when Nearby devices is already
     * allowed for Bluetooth.
     */
    @Command
    fun localNetwork(invoke: Invoke) {
        when {
            Build.VERSION.SDK_INT < ANDROID_17 -> localNetworkAnswered(invoke)
            getPermissionState(LOCAL_NETWORK) == PermissionState.GRANTED -> localNetworkAnswered(invoke)
            else -> requestPermissionForAlias(LOCAL_NETWORK, invoke, "localNetworkAnswered")
        }
    }

    @PermissionCallback
    fun localNetworkAnswered(invoke: Invoke) {
        val allowed = Build.VERSION.SDK_INT < ANDROID_17 || getPermissionState(LOCAL_NETWORK) == PermissionState.GRANTED
        invoke.resolve(JSObject().put("allowed", allowed))
    }

    @Command
    fun release(invoke: Invoke) {
        handBack()
        invoke.resolve()
    }

    @Command
    fun keepAwake(invoke: Invoke) {
        val on = invoke.parseArgs(KeepAwakeArgs::class.java).on
        activity.runOnUiThread {
            if (on) activity.window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            else activity.window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        }
        invoke.resolve()
    }

    /**
     * Back to the phone's usual network now, but Bluetooth stays up until the
     * camera closes it. Dropped at once, the camera couldn't start its Wi-Fi
     * for us again until its connection was ended in its menu (2026-11-01).
     */
    private fun handBack() {
        wifi.release()
        val leaving = camera ?: return
        camera = null
        handingBack = thread(name = "camera-hand-back") {
            leaving.awaitCameraClose(HAND_BACK_WAIT_S)
            leaving.close()
        }
    }

    private fun start(invoke: Invoke) {
        thread(name = "camera-link") {
            val report = report()
            try {
                val args = invoke.parseArgs(ConnectArgs::class.java)
                val client = Client(ByteArray(args.clientId.size) { args.clientId[it].toByte() }, args.clientName)
                link(client, report)
            } catch (stop: Stop) {
                close()
                report.put("failure", stop.sentence)
                stop.detail?.let { report.put("detail", it) }
            } catch (error: Exception) {
                close()
                report.put("failure", "Bluetooth connect failed (${error.javaClass.simpleName}): tap again.")
            }
            invoke.resolve(report)
        }
    }

    /** Marks each step in [report] as it passes, so a failure shows how far it got. */
    private fun link(client: Client, report: JSObject) {
        handingBack?.join()
        handingBack = null
        close()
        blocker()?.let { throw Stop(it) }
        val bonded = CameraGatt.bondedCamera(activity)
        val gatt = CameraGatt.open(activity, bonded ?: findCameraToPair(activity))
        camera = gatt
        report.put("woke", true)
        if (bonded == null) pair(gatt, client.name)
        val access = startWifi(gatt, client.takeIf { bonded == null }) { if ("refused" in it) report.put("registration", it) }
        report.put("wifiStarted", true)
        wifi.join(access.name, access.password)
        report.put("joined", true)
        report.put("address", access.cameraAddress)
    }

    /** Bluetooth needs its own permissions from Android 12; before that, a scan counts as location. */
    private fun permission() = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) NEARBY else LOCATION

    /** What to do before a connect can start; null when it can. */
    private fun blocker(): String? =
        if (CameraGatt.isBluetoothOn(activity)) wifi.blocker else "Turn Bluetooth on, then tap again."

    private fun close() {
        wifi.release()
        camera?.close()
        camera = null
    }

    private fun report() = JSObject().put("woke", false).put("wifiStarted", false).put("joined", false)
}

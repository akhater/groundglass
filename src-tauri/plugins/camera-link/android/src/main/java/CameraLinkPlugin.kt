package co.groundglass.cameralink

import android.Manifest
import android.app.Activity
import android.os.Build
import app.tauri.PermissionState
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import kotlin.concurrent.thread

private const val BLUETOOTH = "bluetooth"

@InvokeArg
class ConnectArgs {
    lateinit var clientId: IntArray
    lateinit var clientName: String
}

/** A step that can't go on; [sentence] tells the user what to do. */
class Stop(val sentence: String) : Exception(sentence)

/** Wakes the paired camera and joins its Wi-Fi. Every outcome resolves: failures are a sentence in the report. */
@TauriPlugin(permissions = [Permission(strings = [Manifest.permission.BLUETOOTH_CONNECT], alias = BLUETOOTH)])
class CameraLinkPlugin(private val activity: Activity) : Plugin(activity) {
    private val wifi = WifiJoin(activity)
    /** Held while connected: the recordings keep Bluetooth up during live view. */
    private var camera: CameraGatt? = null

    /** Whether a tap can start now: the screen dims the button when it can't. */
    @Command
    fun ready(invoke: Invoke) {
        invoke.resolve(JSObject().put("ready", blocker() == null))
    }

    @Command
    fun connect(invoke: Invoke) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S || getPermissionState(BLUETOOTH) == PermissionState.GRANTED) {
            start(invoke)
        } else {
            requestPermissionForAlias(BLUETOOTH, invoke, "permissionAnswered")
        }
    }

    @PermissionCallback
    fun permissionAnswered(invoke: Invoke) {
        if (getPermissionState(BLUETOOTH) == PermissionState.GRANTED) {
            start(invoke)
        } else {
            invoke.resolve(report().put("failure", "Allow Nearby devices for this app, then tap again."))
        }
    }

    @Command
    fun release(invoke: Invoke) {
        close()
        invoke.resolve()
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
            } catch (error: Exception) {
                close()
                report.put("failure", "Bluetooth connect failed (${error.javaClass.simpleName}): tap again.")
            }
            invoke.resolve(report)
        }
    }

    /** Marks each step in [report] as it passes, so a failure shows how far it got. */
    private fun link(client: Client, report: JSObject) {
        close()
        blocker()?.let { throw Stop(it) }
        val gatt = CameraGatt.open(activity)
        camera = gatt
        report.put("woke", true)
        val access = startWifi(gatt, client) { report.put("registration", it) }
        report.put("wifiStarted", true)
        wifi.join(access.name, access.password)
        report.put("joined", true)
        report.put("address", access.cameraAddress)
    }

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

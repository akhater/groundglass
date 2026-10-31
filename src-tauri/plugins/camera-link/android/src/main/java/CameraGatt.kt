package co.groundglass.cameralink

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.content.Context
import java.util.UUID
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

private val CLIENT_CONFIG = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")
/** Bluetooth stacks fail a first connect now and then, without a reason. */
private const val CONNECT_ATTEMPTS = 2
private const val CONNECT_WAIT_S = 12L
private const val ANSWER_WAIT_S = 5L
private const val NOT_ANSWERING = "The camera stopped answering over Bluetooth: tap again."

private class Answer(val ok: Boolean, val value: ByteArray? = null)
/** A notification or indication; a null [uuid] means the link is gone. */
private class Change(val uuid: UUID?, val value: ByteArray)

/**
 * The bonded camera's GATT server, one operation at a time: each call blocks
 * its thread until the camera answers. The calls from before Android 13 still
 * work on every version and the app goes back to Android 7, hence the deprecation.
 */
@SuppressLint("MissingPermission")
@Suppress("DEPRECATION")
class CameraGatt private constructor() {
    private val answers = LinkedBlockingQueue<Answer>()
    private val changes = LinkedBlockingQueue<Change>()
    private var gatt: BluetoothGatt? = null

    private val callback = object : BluetoothGattCallback() {
        override fun onConnectionStateChange(gatt: BluetoothGatt, status: Int, newState: Int) {
            if (status == BluetoothGatt.GATT_SUCCESS && newState == BluetoothProfile.STATE_CONNECTED) {
                gatt.discoverServices()
            } else {
                answers.add(Answer(false))
                changes.add(Change(null, ByteArray(0)))
            }
        }

        override fun onServicesDiscovered(gatt: BluetoothGatt, status: Int) {
            answers.add(Answer(status == BluetoothGatt.GATT_SUCCESS))
        }

        override fun onCharacteristicRead(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic, status: Int) {
            answers.add(Answer(status == BluetoothGatt.GATT_SUCCESS, characteristic.value?.clone()))
        }

        override fun onCharacteristicWrite(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic, status: Int) {
            answers.add(Answer(status == BluetoothGatt.GATT_SUCCESS))
        }

        override fun onDescriptorWrite(gatt: BluetoothGatt, descriptor: BluetoothGattDescriptor, status: Int) {
            answers.add(Answer(status == BluetoothGatt.GATT_SUCCESS))
        }

        override fun onCharacteristicChanged(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic) {
            changes.add(Change(characteristic.uuid, characteristic.value?.clone() ?: ByteArray(0)))
        }
    }

    private fun connect(context: Context, device: BluetoothDevice): Boolean {
        gatt = device.connectGatt(context, false, callback, BluetoothDevice.TRANSPORT_LE)
        val connected = answers.poll(CONNECT_WAIT_S, TimeUnit.SECONDS)?.ok == true
        if (!connected) close()
        return connected
    }

    fun read(uuid: UUID): ByteArray {
        started(link().readCharacteristic(characteristic(uuid)))
        return answer().value ?: ByteArray(0)
    }

    fun write(uuid: UUID, value: ByteArray) {
        val characteristic = characteristic(uuid)
        characteristic.value = value
        characteristic.writeType = BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT
        started(link().writeCharacteristic(characteristic))
        answer()
    }

    fun notifications(uuid: UUID) = subscribe(uuid, BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE)

    fun indications(uuid: UUID) = subscribe(uuid, BluetoothGattDescriptor.ENABLE_INDICATION_VALUE)

    private fun subscribe(uuid: UUID, value: ByteArray) {
        val characteristic = characteristic(uuid)
        started(link().setCharacteristicNotification(characteristic, true))
        val config = characteristic.getDescriptor(CLIENT_CONFIG) ?: throw Stop(NOT_ANSWERING)
        config.value = value
        started(link().writeDescriptor(config))
        answer()
    }

    /** The next change on [uuid] that is [wanted], or null after [seconds]. */
    fun change(uuid: UUID, seconds: Long, wanted: (ByteArray) -> Boolean = { true }): ByteArray? {
        val end = System.nanoTime() + TimeUnit.SECONDS.toNanos(seconds)
        while (true) {
            val change = changes.poll(end - System.nanoTime(), TimeUnit.NANOSECONDS) ?: return null
            if (change.uuid == null) throw Stop(NOT_ANSWERING)
            if (change.uuid == uuid && wanted(change.value)) return change.value
        }
    }

    fun close() {
        gatt?.disconnect()
        gatt?.close()
        gatt = null
    }

    private fun link() = gatt ?: throw Stop(NOT_ANSWERING)

    private fun characteristic(uuid: UUID): BluetoothGattCharacteristic =
        link().services.firstNotNullOfOrNull { it.getCharacteristic(uuid) } ?: throw Stop(NOT_ANSWERING)

    private fun started(accepted: Boolean) {
        if (!accepted) throw Stop(NOT_ANSWERING)
    }

    private fun answer(): Answer {
        val answer = answers.poll(ANSWER_WAIT_S, TimeUnit.SECONDS)
        if (answer?.ok != true) throw Stop(NOT_ANSWERING)
        return answer
    }

    companion object {
        private fun adapter(context: Context) = context.getSystemService(BluetoothManager::class.java).adapter

        /** Needs no permission on any version the app runs on: the manifest covers the older ones. */
        fun isBluetoothOn(context: Context) = adapter(context)?.isEnabled == true

        /** Connects to the camera the phone is paired with, waking it if needed. */
        fun open(context: Context): CameraGatt {
            val device = adapter(context)?.bondedDevices?.firstOrNull { isCamera(it) }
                ?: throw Stop("No paired camera: pair it once with Canon's Camera Connect, then tap again.")
            repeat(CONNECT_ATTEMPTS) {
                val camera = CameraGatt()
                if (camera.connect(context, device)) return camera
            }
            throw Stop("The camera didn't answer over Bluetooth: check it isn't switched off, then tap again.")
        }

        /** The name the camera advertises, or the service the phone remembers from it. */
        private fun isCamera(device: BluetoothDevice) =
            device.name?.startsWith("EOS") == true || device.uuids?.any { it.uuid == CANON_SERVICE } == true
    }
}

package co.groundglass.cameralink

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothManager
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.os.ParcelUuid
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

private const val CANON_COMPANY_ID = 0x01a9
private const val SCAN_WAIT_S = 12L
private const val NO_CAMERA = "No camera found: open its pairing screen, then tap again."
private const val NOT_READY = "The camera isn't on its pairing screen: open it, then tap again."

/**
 * Finds a camera showing its pairing screen. In the recordings its advert holds
 * six bytes after Canon's id: the fourth and fifth are zero on the pairing
 * screen, and the same non-zero pair at every other time, on or asleep.
 */
private fun isOnPairingScreen(advert: ByteArray) = advert.size >= 5 && advert[3] == 0.toByte() && advert[4] == 0.toByte()

@SuppressLint("MissingPermission")
fun findCameraToPair(context: Context): BluetoothDevice {
    val scanner = context.getSystemService(BluetoothManager::class.java).adapter?.bluetoothLeScanner ?: throw Stop(NO_CAMERA)
    val found = LinkedBlockingQueue<BluetoothDevice>()
    var seen = false
    val callback = object : ScanCallback() {
        override fun onScanResult(callbackType: Int, result: ScanResult) {
            seen = true
            val advert = result.scanRecord?.getManufacturerSpecificData(CANON_COMPANY_ID) ?: return
            if (isOnPairingScreen(advert)) found.add(result.device)
        }
    }
    val filter = ScanFilter.Builder().setServiceUuid(ParcelUuid(CANON_SERVICE)).build()
    val settings = ScanSettings.Builder().setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY).build()
    scanner.startScan(listOf(filter), settings, callback)
    try {
        return found.poll(SCAN_WAIT_S, TimeUnit.SECONDS) ?: throw Stop(if (seen) NOT_READY else NO_CAMERA)
    } finally {
        scanner.stopScan(callback)
    }
}

package co.groundglass.cameralink

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.wifi.WifiManager
import android.net.wifi.WifiNetworkSpecifier
import android.os.Build
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

/** Long enough to answer the system's dialog the first time; it doesn't come back after that. */
private const val JOIN_WAIT_MS = 60_000

/** Joins the camera's Wi-Fi for this app alone: the phone itself stays on its usual network. */
class WifiJoin(context: Context) {
    private val connectivity = context.getSystemService(ConnectivityManager::class.java)
    private val wifi = context.applicationContext.getSystemService(WifiManager::class.java)
    private var request: ConnectivityManager.NetworkCallback? = null

    val isOn: Boolean get() = wifi.isWifiEnabled

    /** Blocks until joined, with all of this process's sockets (Rust's too) bound to that network. */
    fun join(name: String, password: String) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) throw Stop("One-tap connect needs Android 10 or later.")
        val joined = LinkedBlockingQueue<Boolean>()
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                joined.add(connectivity.bindProcessToNetwork(network))
            }

            override fun onUnavailable() {
                joined.add(false)
            }

            override fun onLost(network: Network) {
                connectivity.bindProcessToNetwork(null)
            }
        }
        val specifier = WifiNetworkSpecifier.Builder().setSsid(name).setWpa2Passphrase(password).build()
        val network = NetworkRequest.Builder()
            .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
            .removeCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .setNetworkSpecifier(specifier)
            .build()
        connectivity.requestNetwork(network, callback, JOIN_WAIT_MS)
        request = callback
        // Android answers by JOIN_WAIT_MS; the margin only makes sure this never hangs.
        if (joined.poll(JOIN_WAIT_MS + 5_000L, TimeUnit.MILLISECONDS) != true) {
            throw Stop("The phone didn't join the camera's Wi-Fi: tap again, and choose Connect if it asks.")
        }
    }

    /** Back to the phone's usual network. */
    fun release() {
        request?.let { connectivity.unregisterNetworkCallback(it) }
        request = null
        connectivity.bindProcessToNetwork(null)
    }
}

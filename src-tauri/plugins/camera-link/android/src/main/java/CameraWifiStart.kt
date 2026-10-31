package co.groundglass.cameralink

import java.util.UUID

/** Canon's Bluetooth protocol, as Camera Connect uses it on an EOS R6 Mark III (recorded 2026-10-31). */
fun canon(id: String): UUID = UUID.fromString("$id-0000-1000-0000-d8492fffa821")
private fun standard(id: String) = UUID.fromString("0000$id-0000-1000-8000-00805f9b34fb")

val CANON_SERVICE: UUID = canon("00010000")
private val DEVICE_INFORMATION = listOf("2a29", "2a24", "2a26", "2a28", "2a25").map(::standard)
private val STATUS = listOf("00030001", "00030002", "00030011", "00030021", "00030031").map(::canon)
private val SETUP_COMMAND = canon("0001000a")
private val SETUP_REPLY = canon("0001000c")
private val SESSION_COMMAND = canon("00040002")
private val SESSION_REPLY = canon("00040003")
/** Write 0a: the camera's address comes back as a notification. Write 01: Wi-Fi starts. */
private val WIFI_COMMAND = canon("00020002")
private val WIFI_STATE = canon("00020003")
private val WIFI_NAME = canon("00020004")
private val WIFI_DETAILS = canon("00020005")
private val WIFI_PASSWORD = canon("00020006")

private val SETUP_CODES = byteArrayOf(0x01, 0x06, 0x07, 0x08, 0x0c)
private const val CLIENT_ID_CODE: Byte = 0x03
private const val CLIENT_NAME_CODE: Byte = 0x04
private val CLIENT_MODE = byteArrayOf(0x05, 0x02)
private val SESSION_START = byteArrayOf(0x05, 0, 0, 0, 0, 0, 0, 0)
private val ASK_ADDRESS = byteArrayOf(0x0a)
private val START_WIFI = byteArrayOf(0x01)
/** After `01 03`, about 0.8 s later. */
private val WIFI_RUNNING = byteArrayOf(0x02, 0x03)
private const val LAST_SETUP_REPLY: Byte = 0x07

private const val REPLY_WAIT_S = 3L
/** Recorded at 0.75-0.95 s. */
private const val WIFI_START_WAIT_S = 8L
/** Camera Connect wrote `01` 4 to 28 s after setup, when asked for live view; the shortest safe gap isn't known. */
private const val SETUP_SETTLE_MS = 2000L
private const val NO_WIFI = "The camera didn't start its Wi-Fi: close Canon's app, then tap again."

class WifiAccess(val name: String, val password: String, val cameraAddress: String)

/** Who will log in over PTP/IP: the id and name its Init sends. */
class Client(val id: ByteArray, val name: String)

/**
 * The steps of a bonded reconnect in Camera Connect's order, then the write
 * that starts Wi-Fi. Which of them the camera requires isn't known yet. Right
 * after pairing there is a [client] to register, and [onRegistered] hears how
 * the camera took it; a plain reconnect has none, as in the recordings.
 */
fun startWifi(camera: CameraGatt, client: Client?, onRegistered: (String) -> Unit): WifiAccess {
    camera.read(canon("0001000b"))
    DEVICE_INFORMATION.forEach { camera.read(it) }
    camera.read(canon("00010005"))
    camera.indications(SETUP_REPLY)
    camera.read(canon("00020001"))
    camera.indications(WIFI_STATE)
    camera.notifications(WIFI_COMMAND)
    camera.read(STATUS.first())
    STATUS.forEach { camera.notifications(it) }
    camera.read(canon("00040001"))
    camera.read(SESSION_REPLY)
    camera.indications(SESSION_REPLY)
    client?.let { onRegistered(register(camera, it)) }

    camera.write(WIFI_COMMAND, ASK_ADDRESS)
    val address = camera.change(WIFI_COMMAND, REPLY_WAIT_S)?.let(::cameraAddress) ?: throw Stop(NO_WIFI)
    camera.write(SESSION_COMMAND, SESSION_START)
    camera.change(SESSION_REPLY, REPLY_WAIT_S) ?: throw Stop(NO_WIFI)

    val name = text(camera.read(WIFI_NAME))
    val password = text(camera.read(WIFI_PASSWORD))
    camera.read(WIFI_DETAILS)
    if (name.isEmpty() || password.isEmpty()) throw Stop(NO_WIFI)

    SETUP_CODES.forEach { camera.write(SETUP_COMMAND, byteArrayOf(it)) }
    camera.change(SETUP_REPLY, REPLY_WAIT_S) { it.firstOrNull() == LAST_SETUP_REPLY }
    Thread.sleep(SETUP_SETTLE_MS)

    camera.write(WIFI_COMMAND, START_WIFI)
    camera.change(WIFI_STATE, WIFI_START_WAIT_S) { it.contentEquals(WIFI_RUNNING) } ?: throw Stop(NO_WIFI)
    return WifiAccess(name, password, address)
}

/**
 * Camera Connect writes these once, when pairing, just before asking for the
 * address: its 16-byte id, then its name as text, then `05 02`. On a plain
 * reconnect the camera accepts the writes and still refuses Init. A refusal
 * doesn't stop the steps after it, so a test shows how far the camera goes.
 * Returns how each write went.
 */
private fun register(camera: CameraGatt, client: Client): String {
    val writes = listOf(
        "id" to byteArrayOf(CLIENT_ID_CODE) + client.id,
        "name" to byteArrayOf(CLIENT_NAME_CODE) + client.name.toByteArray(Charsets.UTF_8),
        "mode" to CLIENT_MODE,
    )
    return writes.joinToString(", ") { (label, value) ->
        val outcome = if (camera.tryWrite(SETUP_COMMAND, value)) "ok" else "refused"
        "$label $outcome"
    }
}

/** `7f 10 10 04`, then the IPv4 address lowest byte first, then more this doesn't need. */
private fun cameraAddress(answer: ByteArray): String? {
    if (answer.size < 8 || answer[3] != 4.toByte()) return null
    return (7 downTo 4).joinToString(".") { (answer[it].toInt() and 0xff).toString() }
}

/** The camera pads text with zeros. */
private fun text(value: ByteArray) = String(value.takeWhile { it != 0.toByte() }.toByteArray(), Charsets.UTF_8)

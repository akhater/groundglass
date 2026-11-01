package co.groundglass.cameralink

private val PAIRING_NAME = canon("00010006")
private const val PAIRING_NAME_CODE: Byte = 0x01
private const val PAIRING_ACCEPTED: Byte = 0x02
/** Recorded at 1.6 s. */
private const val PAIRING_WAIT_S = 10L
private const val REFUSED =
    "The camera didn't accept the pairing: forget it in the phone's Bluetooth settings, open its pairing screen, then tap again."

/**
 * First-time pairing, as recorded from Camera Connect with the camera on its
 * pairing screen: Android's bond, then the app's name, which the camera accepts
 * with an indication. startWifi then registers the client.
 */
fun pair(camera: CameraGatt, name: String) {
    camera.bond()
    camera.write(PAIRING_NAME, byteArrayOf(PAIRING_NAME_CODE) + name.toByteArray(Charsets.UTF_8))
    camera.indications(PAIRING_NAME)
    camera.change(PAIRING_NAME, PAIRING_WAIT_S) { it.firstOrNull() == PAIRING_ACCEPTED } ?: throw Stop(REFUSED)
}

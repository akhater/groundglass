// One-tap connect: Bluetooth wakes the camera and joins its Wi-Fi, then one
// attempt over the normal path. Only live view's own loop retries, and that
// starts only once this has worked: the camera mustn't get PTP/IP attempts in a row.
import { useEffect, useRef, useState } from "react";
import * as camera from "./camera";
import { liveViewStopped } from "./liveView";
import { oneTapReport, type OneTapReport } from "./oneTapReport";
import { restLeft } from "./oneTapRest";

/** The camera's own words: `describe` is written for an address typed by hand. */
function rawError(error: unknown): string {
  const reason = camera.refusalReason(error);
  if (reason !== null) return `refused to pair, reason 0x${reason.toString(16)}`;
  return (error as { detail?: string })?.detail ?? camera.describe(error);
}

async function reach(address: string, cameraDisplayOn: boolean, link: camera.BluetoothLink, onPairing: () => void) {
  let info: camera.CameraInfo;
  try {
    info = await camera.connect(address, onPairing);
  } catch (error) {
    const survey = await camera.cameraSurvey(address);
    // Only PTP/IP can refuse a pairing, and a refusal is an answer.
    const refused = camera.refusalReason(error) ?? undefined;
    const protocol = refused === undefined ? undefined : "PTP/IP";
    return { info: null, report: oneTapReport(link, { protocol, refused, live: false, error: rawError(error), survey }) };
  }
  try {
    await camera.startLiveView(cameraDisplayOn);
  } catch (error) {
    await camera.stopLiveView().catch(() => {}); // ends the PTP session
    return { info: null, report: oneTapReport(link, { protocol: info.protocol, live: false, error: rawError(error) }) };
  }
  return { info, report: oneTapReport(link, { protocol: info.protocol, live: true }) };
}

/** `onLive` runs with the session already open and live view started. */
export function useOneTap(cameraDisplayOn: boolean, onLive: (address: string, info: camera.CameraInfo) => void) {
  const [working, setWorking] = useState(false);
  const [hint, setHint] = useState("");
  const [report, setReport] = useState<OneTapReport | null>(null);
  /** Off while Bluetooth or Wi-Fi is off: the button dims, and a tap says what to turn on. */
  const [ready, setReady] = useState(true);
  const linked = useRef(false);
  /** When Stop was pressed: the camera needs a rest before its Wi-Fi can start again. */
  const stoppedAt = useRef<number | null>(null);

  useEffect(() => {
    if (!camera.canConnectOverBluetooth) return;
    const check = () => void camera.bluetoothReady().then(setReady, () => setReady(false));
    check();
    // Coming back from the settings, or from the quick settings over the app.
    document.addEventListener("visibilitychange", check);
    window.addEventListener("focus", check);
    return () => {
      document.removeEventListener("visibilitychange", check);
      window.removeEventListener("focus", check);
    };
  }, []);

  const release = () => {
    if (!linked.current) return Promise.resolve();
    linked.current = false;
    return camera.bluetoothRelease().catch(() => {});
  };

  const start = async () => {
    setWorking(true);
    setHint("");
    setReport(null);
    linked.current = false;
    try {
      await liveViewStopped(); // a lost or hidden session may still be closing
      const rest = restLeft(stoppedAt.current, performance.now());
      if (rest > 0) {
        setHint("The camera needs a moment after Stop.");
        await new Promise((resolve) => setTimeout(resolve, rest));
        setHint("");
      }
      const link = await camera.bluetoothConnect();
      if (!link.address) {
        setReport(oneTapReport(link));
        return;
      }
      linked.current = true;
      const { info, report } = await reach(link.address, cameraDisplayOn, link, () =>
        setHint("First time? Press OK on the camera to pair."),
      );
      setReport(report);
      if (info) onLive(link.address, info);
      else await release();
    } catch (error) {
      await release();
      setReport({ steps: [], message: `Bluetooth connect failed: ${camera.describe(error)}`, detail: null });
    } finally {
      setWorking(false);
    }
  };

  /** After Stop: once live view has stopped, the phone's traffic goes back to normal. */
  const stop = () => {
    setReport(null);
    if (linked.current) stoppedAt.current = performance.now();
    void liveViewStopped().then(release);
  };

  /**
   * A one-tap session lost, or the app back from hiding: the whole tap again,
   * so the camera's Wi-Fi starts and the phone joins it anew, then one Init.
   * If that fails the report says what to do. False if this isn't such a session.
   */
  const resume = () => {
    if (!linked.current) return false;
    void start();
    return true;
  };

  return { ready, working, hint, report, start, stop, resume, hideReport: () => setReport(null) };
}

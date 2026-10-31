// One-tap connect: Bluetooth wakes the camera and joins its Wi-Fi, then one
// attempt over the normal path. Only live view's own loop retries, and that
// starts only once this has worked: the camera mustn't get PTP/IP attempts in a row.
import { useRef, useState } from "react";
import * as camera from "./camera";
import { liveViewStopped } from "./liveView";
import { oneTapReport, type OneTapReport } from "./oneTapReport";

/** The camera's own words: `describe` is written for an address typed by hand. */
function rawError(error: unknown): string {
  return (error as { detail?: string })?.detail ?? camera.describe(error);
}

async function reach(address: string, cameraDisplayOn: boolean, link: camera.BluetoothLink, onPairing: () => void) {
  let info: camera.CameraInfo;
  try {
    info = await camera.connect(address, onPairing);
  } catch (error) {
    const survey = await camera.cameraSurvey(address);
    return { info: null, report: oneTapReport(link, { live: false, error: rawError(error), survey }) };
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
  const linked = useRef(false);

  const release = () => {
    if (!linked.current) return Promise.resolve();
    linked.current = false;
    return camera.bluetoothRelease().catch(() => {});
  };

  const start = async () => {
    setWorking(true);
    setHint("");
    setReport(null);
    try {
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
    void liveViewStopped().then(release);
  };

  return { working, hint, report, start, stop, hideReport: () => setReport(null) };
}

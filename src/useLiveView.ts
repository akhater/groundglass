// Runs live view while it is started and the app is on screen, so the camera's
// screen comes back on whenever Groundglass is in the background.
import { useEffect, useState, type RefObject } from "react";
import type * as camera from "./camera";
import type { Cube } from "./cube";
import { runLiveView, type Status } from "./liveView";
import type { Peaking } from "./render";

type Options = {
  canvas: RefObject<HTMLCanvasElement | null>;
  running: boolean;
  address: string;
  /** A session already open, used by the next run and cleared. */
  connected: RefObject<camera.CameraInfo | null>;
  /** True when one-tap connect takes over from a run with no session, or with `noPicture` from one with no frames. */
  resume: RefObject<(noPicture: boolean) => boolean>;
  /** Read on every frame, so changing them needs no restart. */
  cameraDisplayOn: RefObject<boolean>;
  zebraPercent: RefObject<number | null>;
  look: RefObject<Cube | null>;
  peaking: RefObject<Peaking | null>;
  zoom: RefObject<camera.Zoom>;
  turns: RefObject<number>;
};

/** The live view's status; null while stopped. */
export function useLiveView({ canvas, running, address, connected, resume, cameraDisplayOn, zebraPercent, look, peaking, zoom, turns }: Options) {
  const [visible, setVisible] = useState(!document.hidden);
  const [status, setStatus] = useState<Status | null>(null);

  useEffect(() => {
    const update = () => setVisible(!document.hidden);
    document.addEventListener("visibilitychange", update);
    return () => document.removeEventListener("visibilitychange", update);
  }, []);

  useEffect(() => {
    if (!running || !visible || !canvas.current) return;
    const abort = new AbortController();
    setStatus(null);
    void runLiveView({
      canvas: canvas.current,
      address,
      cameraDisplayOn: () => cameraDisplayOn.current,
      connected: () => {
        const info = connected.current;
        connected.current = null;
        return info;
      },
      resume: (noPicture) => resume.current(noPicture),
      zebraPercent: () => zebraPercent.current,
      look: () => look.current,
      peaking: () => peaking.current,
      zoom: () => zoom.current,
      turns: () => turns.current,
      onStatus: setStatus,
      signal: abort.signal,
    });
    return () => abort.abort();
  }, [canvas, running, visible, address, connected, resume, cameraDisplayOn, zebraPercent, look, peaking, zoom, turns]);

  return running ? status : null;
}

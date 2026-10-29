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
  cameraDisplayOn: boolean;
  /** Read on every frame, so changing them needs no restart. */
  zebraPercent: RefObject<number | null>;
  look: RefObject<Cube | null>;
  peaking: RefObject<Peaking | null>;
  zoom: RefObject<camera.Zoom>;
  turns: RefObject<number>;
};

/** The live view's status; null while stopped. */
export function useLiveView({ canvas, running, address, cameraDisplayOn, zebraPercent, look, peaking, zoom, turns }: Options) {
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
      cameraDisplayOn,
      zebraPercent: () => zebraPercent.current,
      look: () => look.current,
      peaking: () => peaking.current,
      zoom: () => zoom.current,
      turns: () => turns.current,
      onStatus: setStatus,
      signal: abort.signal,
    });
    return () => abort.abort();
  }, [canvas, running, visible, address, cameraDisplayOn, zebraPercent, look, peaking, zoom, turns]);

  return running ? status : null;
}

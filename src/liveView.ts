// The live view loop: one frame in flight, drawn as soon as it arrives.
// Any failure restarts live view; a lost camera is retried until stopped.
import * as camera from "./camera";
import type { Cube } from "./cube";
import { createRenderer, type Peaking } from "./render";

export type Status =
  | { state: "connecting"; hint?: string }
  | { state: "live"; fps: number; camera: camera.CameraInfo }
  | { state: "lost"; message: string };

export type LiveViewOptions = {
  canvas: HTMLCanvasElement;
  address: string;
  cameraDisplayOn: boolean;
  /** Read on every frame, so changing it needs no restart. Null = zebra off. */
  zebraPercent: () => number | null;
  /** Read on every frame too. Null = the camera's picture as it is. */
  look: () => Cube | null;
  /** Read on every frame too. Null = peaking off. */
  peaking: () => Peaking | null;
  /** Read on every frame too: the magnification asked of the camera. */
  zoom: () => camera.Zoom;
  onStatus: (status: Status) => void;
  signal: AbortSignal;
};

const RETRY_MS = 2000;
/** In EOS Utility mode, a connect this slow is the camera asking to pair (the first time). */
const PAIRING_HINT_MS = 2000;
/** While the camera takes a photo it sends no frames: ask again this often. */
const BUSY_MS = 100;

/** The previous run, so a new one starts only after the old one has stopped the camera. */
let previousRun: Promise<void> = Promise.resolve();

/** Runs until `signal` aborts, then stops live view and turns the camera's screen back on. */
export function runLiveView(options: LiveViewOptions): Promise<void> {
  const run = previousRun.then(() => loop(options));
  previousRun = run.catch(() => {});
  return run;
}

async function loop({ canvas, address, cameraDisplayOn, zebraPercent, look, peaking, zoom, onStatus, signal }: LiveViewOptions) {
  const renderer = createRenderer(canvas);
  let info: camera.CameraInfo | null = null;
  while (!signal.aborted) {
    try {
      onStatus({ state: "connecting" });
      if (!info) {
        let hint = 0;
        let done = false;
        const showHint = () => onStatus({ state: "connecting", hint: "First time? Press OK on the camera to pair." });
        try {
          info = await camera.connect(address, () => {
            if (!done) hint = window.setTimeout(showHint, PAIRING_HINT_MS);
          });
        } finally {
          done = true;
          clearTimeout(hint);
        }
      }
      await camera.startLiveView(cameraDisplayOn);
      let frames = 0;
      let since = performance.now();
      while (!signal.aborted) {
        let jpeg: ArrayBuffer;
        try {
          jpeg = await camera.frame();
        } catch (error) {
          // Busy (a photo being taken): live view resumes by itself.
          if (!camera.isBusy(error)) throw error;
          await sleep(BUSY_MS, signal);
          continue;
        }
        const decoded = await createImageBitmap(new Blob([jpeg], { type: "image/jpeg" }));
        const image = zoom() === 10 ? await centralHalf(decoded) : decoded;
        renderer.draw(image, zebraPercent(), look(), peaking());
        image.close();
        frames++;
        const now = performance.now();
        if (now - since >= 1000) {
          onStatus({ state: "live", fps: Math.round((frames * 1000) / (now - since)), camera: info });
          frames = 0;
          since = now;
        }
      }
    } catch (error) {
      if (signal.aborted) break;
      if (camera.isRestartable(error)) continue; // restart live view at once
      info = null;
      // No picture while the camera is away: an old frame would pass for live.
      renderer.clear();
      onStatus({ state: "lost", message: camera.describe(error) });
      await sleep(RETRY_MS, signal);
    }
  }
  renderer.clear();
  await camera.stopLiveView().catch(() => {});
}

/** x10: the camera sends the x5 pixels, so show their central half. */
async function centralHalf(image: ImageBitmap): Promise<ImageBitmap> {
  const { width, height } = image;
  const half = await createImageBitmap(image, width / 4, height / 4, width / 2, height / 2);
  image.close();
  return half;
}

function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    const timer = setTimeout(resolve, ms);
    signal.addEventListener("abort", () => {
      clearTimeout(timer);
      resolve();
    });
  });
}

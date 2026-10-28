// The live view loop: one frame in flight, drawn as soon as it arrives.
// Any failure restarts live view; a lost camera is retried until stopped.
import * as camera from "./camera";
import type { Cube } from "./cube";
import { createRenderer, type Peaking } from "./render";

export type Status =
  | { state: "connecting" }
  | { state: "live"; fps: number }
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
  onStatus: (status: Status) => void;
  signal: AbortSignal;
};

const RETRY_MS = 2000;

/** The previous run, so a new one starts only after the old one has stopped the camera. */
let previousRun: Promise<void> = Promise.resolve();

/** Runs until `signal` aborts, then stops live view and turns the camera's screen back on. */
export function runLiveView(options: LiveViewOptions): Promise<void> {
  const run = previousRun.then(() => loop(options));
  previousRun = run.catch(() => {});
  return run;
}

async function loop({ canvas, address, cameraDisplayOn, zebraPercent, look, peaking, onStatus, signal }: LiveViewOptions) {
  const renderer = createRenderer(canvas);
  let connected = false;
  while (!signal.aborted) {
    try {
      onStatus({ state: "connecting" });
      if (!connected) {
        await camera.connect(address);
        connected = true;
      }
      await camera.startLiveView(cameraDisplayOn);
      let frames = 0;
      let since = performance.now();
      while (!signal.aborted) {
        const jpeg = await camera.frame();
        const image = await createImageBitmap(new Blob([jpeg], { type: "image/jpeg" }));
        renderer.draw(image, zebraPercent(), look(), peaking());
        image.close();
        frames++;
        const now = performance.now();
        if (now - since >= 1000) {
          onStatus({ state: "live", fps: Math.round((frames * 1000) / (now - since)) });
          frames = 0;
          since = now;
        }
      }
    } catch (error) {
      if (signal.aborted) break;
      if (camera.isRestartable(error)) continue; // restart live view at once
      connected = false;
      onStatus({ state: "lost", message: camera.describe(error) });
      await sleep(RETRY_MS, signal);
    }
  }
  await camera.stopLiveView().catch(() => {});
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

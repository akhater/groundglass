// The camera's battery, asked for rarely while live: it changes slowly, and
// over CCAPI each reading is a request.
import { useEffect, useState } from "react";
import * as camera from "./camera";

/** The first reading waits for the camera's first reports (EOS Utility mode). */
const FIRST_MS = 2_000;
const EVERY_MS = 30_000;

/** The charge, or null while not live, before the first answer or when the camera doesn't say. */
export function useBattery(live: boolean) {
  const [battery, setBattery] = useState<camera.Battery | null>(null);

  useEffect(() => {
    setBattery(null);
    if (!live) return;
    let running = true;
    let timer = 0;
    let wake = () => {};
    void (async () => {
      let wait = FIRST_MS;
      while (running) {
        await new Promise<void>((resolve) => {
          wake = resolve;
          timer = window.setTimeout(resolve, wait);
        });
        // Never ask once live view has stopped: after a power cycle that request
        // would open the camera's new CCAPI session before GET /ccapi (Err 70).
        if (!running) break;
        wait = EVERY_MS;
        const next = await camera.battery().catch(() => undefined);
        if (running && next !== undefined) setBattery(next);
      }
    })();
    return () => {
      running = false;
      clearTimeout(timer);
      wake();
    };
  }, [live]);

  return battery;
}

// The camera's battery, asked for rarely while live: it changes slowly, and
// over CCAPI each reading is a request.
import { useEffect, useState } from "react";
import * as camera from "./camera";

/** The first reading waits for the camera's first reports (EOS Utility mode). */
const FIRST_MS = 2_000;
const EVERY_MS = 30_000;

/** Percent, or null while not live, before the first answer or when the camera doesn't say. */
export function useBattery(live: boolean) {
  const [percent, setPercent] = useState<number | null>(null);

  useEffect(() => {
    setPercent(null);
    if (!live) return;
    let running = true;
    void (async () => {
      let wait = FIRST_MS;
      while (running) {
        await new Promise((resolve) => setTimeout(resolve, wait));
        wait = EVERY_MS;
        const next = await camera.battery().catch(() => undefined);
        if (running && next !== undefined) setPercent(next);
      }
    })();
    return () => {
      running = false;
    };
  }, [live]);

  return percent;
}

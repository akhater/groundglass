// The camera's electronic level, asked for a few times a second while shown.
import { useEffect, useState } from "react";
import * as camera from "./camera";

const POLL_MS = 250;

/** The latest level, or null while off or before the camera has given one. */
export function useLevel(on: boolean) {
  const [level, setLevel] = useState<camera.Level | null>(null);

  useEffect(() => {
    setLevel(null);
    if (!on) return;
    let running = true;
    void (async () => {
      while (running) {
        const next = await camera.level().catch(() => null);
        if (running && next) setLevel(next);
        await new Promise((resolve) => setTimeout(resolve, POLL_MS));
      }
    })();
    return () => {
      running = false;
    };
  }, [on]);

  return level;
}

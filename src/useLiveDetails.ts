// The camera's level, AF frames and histogram, asked for a few times a second while live.
import { useEffect, useState } from "react";
import * as camera from "./camera";

const POLL_MS = 250;

/** The latest details, or null while not live or before the first answer. */
export function useLiveDetails(live: boolean) {
  const [details, setDetails] = useState<camera.LiveDetails | null>(null);

  useEffect(() => {
    setDetails(null);
    if (!live) return;
    let running = true;
    void (async () => {
      while (running) {
        const next = await camera.liveDetails().catch(() => null);
        if (running && next) setDetails(next);
        await new Promise((resolve) => setTimeout(resolve, POLL_MS));
      }
    })();
    return () => {
      running = false;
    };
  }, [live]);

  return details;
}

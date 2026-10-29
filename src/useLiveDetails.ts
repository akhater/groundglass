// The camera's level, AF frames and histogram, asked for a few times a second while live.
import { useEffect, useRef, useState, type RefObject } from "react";
import * as camera from "./camera";

const POLL_MS = 250;

/**
 * The latest details, or null while not live or before the first answer.
 * `locked` (rotation lock) is read on every request: the picture keeps its turn,
 * across a restart of live view too.
 */
export function useLiveDetails(live: boolean, locked: RefObject<boolean>) {
  const [details, setDetails] = useState<camera.LiveDetails | null>(null);
  const turns = useRef(0);

  useEffect(() => {
    setDetails(null);
    if (!live) return;
    let running = true;
    void (async () => {
      while (running) {
        const next = await camera.liveDetails(turns.current, locked.current).catch(() => null);
        if (next) turns.current = next.turns;
        if (running && next) setDetails(next);
        await new Promise((resolve) => setTimeout(resolve, POLL_MS));
      }
    })();
    return () => {
      running = false;
    };
  }, [live, locked]);

  return details;
}

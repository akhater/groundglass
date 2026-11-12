// Whole seconds since `running` turned on, counting up once a second; 0 while off.
// Shown while connecting, so a long wait reads as still working.
import { useEffect, useState } from "react";

export function useElapsed(running: boolean) {
  const [seconds, setSeconds] = useState(0);

  useEffect(() => {
    setSeconds(0);
    if (!running) return;
    const started = Date.now();
    const tick = setInterval(() => setSeconds(Math.floor((Date.now() - started) / 1000)), 1000);
    return () => clearInterval(tick);
  }, [running]);

  return seconds;
}

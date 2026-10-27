import { useEffect, useRef, useState } from "react";
import { runLiveView, type Status } from "./liveView";

const ADDRESS_KEY = "groundglass.address";

export default function App() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [address, setAddress] = useState(() => remembered(ADDRESS_KEY) ?? "");
  const [cameraDisplayOn, setCameraDisplayOn] = useState(false);
  const [running, setRunning] = useState(false);
  const [visible, setVisible] = useState(!document.hidden);
  const [status, setStatus] = useState<Status | null>(null);
  const [zebraOn, setZebraOn] = useState(true);
  const [zebraPercent, setZebraPercent] = useState(98);
  // The loop reads this on every frame, so zebra changes need no restart.
  const zebra = useRef<number | null>(98);
  zebra.current = zebraOn ? zebraPercent : null;

  // Live view runs only while the app is on screen, so the camera's
  // screen comes back on whenever Groundglass is in the background.
  useEffect(() => {
    const update = () => setVisible(!document.hidden);
    document.addEventListener("visibilitychange", update);
    return () => document.removeEventListener("visibilitychange", update);
  }, []);

  useEffect(() => {
    if (!running || !visible || !canvas.current) return;
    const abort = new AbortController();
    void runLiveView({
      canvas: canvas.current,
      address,
      cameraDisplayOn,
      zebraPercent: () => zebra.current,
      onStatus: setStatus,
      signal: abort.signal,
    });
    return () => abort.abort();
  }, [running, visible, address, cameraDisplayOn]);

  const start = () => {
    remember(ADDRESS_KEY, address);
    setRunning(true);
  };

  return (
    <main className="app">
      <canvas ref={canvas} className="view" />
      <div className="bar">
        {running ? (
          <>
            <span className="status">{label(status)}</span>
            <label className="toggle">
              <input
                type="checkbox"
                checked={cameraDisplayOn}
                onChange={(e) => setCameraDisplayOn(e.target.checked)}
              />
              Camera screen
            </label>
            <label className="toggle">
              <input type="checkbox" checked={zebraOn} onChange={(e) => setZebraOn(e.target.checked)} />
              Zebra {zebraPercent}%
            </label>
            <input
              type="range"
              min={70}
              max={100}
              value={zebraPercent}
              disabled={!zebraOn}
              onChange={(e) => setZebraPercent(Number(e.target.value))}
              aria-label="Zebra level"
            />
            <button onClick={() => setRunning(false)}>Stop</button>
          </>
        ) : (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              start();
            }}
          >
            <input
              value={address}
              onChange={(e) => setAddress(e.target.value)}
              placeholder="Camera address, e.g. 192.168.1.2:8080"
              inputMode="url"
              autoFocus
            />
            <button type="submit" disabled={!address.trim()}>
              Start
            </button>
          </form>
        )}
      </div>
    </main>
  );
}

function label(status: Status | null): string {
  switch (status?.state) {
    case "live":
      return `Live · ${status.fps} fps`;
    case "lost":
      return `${status.message} Retrying…`;
    default:
      return "Connecting…";
  }
}

function remembered(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function remember(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Only a convenience: the address is typed again next time.
  }
}

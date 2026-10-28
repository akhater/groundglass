import { useEffect, useRef, useState } from "react";
import * as camera from "./camera";
import { parseCube, type Cube } from "./cube";
import { Exposure } from "./Exposure";
import { runLiveView, type Status } from "./liveView";

const ADDRESS_KEY = "groundglass.address";
const LOOK_KEY = "groundglass.look";

export default function App() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [address, setAddress] = useState(() => remembered(ADDRESS_KEY) ?? "");
  // On by default: with the camera's screen off, its own controls are locked too.
  const [cameraDisplayOn, setCameraDisplayOn] = useState(true);
  const [running, setRunning] = useState(false);
  const [visible, setVisible] = useState(!document.hidden);
  const [status, setStatus] = useState<Status | null>(null);
  const [zebraOn, setZebraOn] = useState(true);
  const [zebraPercent, setZebraPercent] = useState(98);
  const [looks, setLooks] = useState<string[]>([]);
  const [lookName, setLookName] = useState(() => remembered(LOOK_KEY) ?? "");
  const [lookError, setLookError] = useState("");
  const [shot, setShot] = useState<"idle" | "busy" | "done" | string>("idle");
  // The loop reads these on every frame, so changing them needs no restart.
  const zebra = useRef<number | null>(null);
  zebra.current = zebraOn ? zebraPercent : null;
  const look = useRef<Cube | null>(null);

  // Live view runs only while the app is on screen, so the camera's
  // screen comes back on whenever Groundglass is in the background.
  useEffect(() => {
    const update = () => setVisible(!document.hidden);
    document.addEventListener("visibilitychange", update);
    return () => document.removeEventListener("visibilitychange", update);
  }, []);

  useEffect(() => {
    camera.listLuts().then(setLooks, (e) => setLookError(String(e)));
  }, []);

  useEffect(() => {
    remember(LOOK_KEY, lookName);
    look.current = null;
    if (!lookName) return;
    let current = true;
    camera
      .readLut(lookName)
      .then((text) => {
        if (current) look.current = parseCube(text, lookName);
        setLookError("");
      })
      .catch((e) => setLookError(`${lookName}: ${e}`));
    return () => {
      current = false;
    };
  }, [lookName]);

  useEffect(() => {
    if (!running || !visible || !canvas.current) return;
    const abort = new AbortController();
    void runLiveView({
      canvas: canvas.current,
      address,
      cameraDisplayOn,
      zebraPercent: () => zebra.current,
      look: () => look.current,
      onStatus: setStatus,
      signal: abort.signal,
    });
    return () => abort.abort();
  }, [running, visible, address, cameraDisplayOn]);

  async function shoot() {
    if (shot === "busy") return;
    setShot("busy");
    try {
      await camera.shoot();
      setShot("done");
    } catch (e) {
      setShot(camera.describe(e));
    }
    setTimeout(() => setShot("idle"), 2000);
  }

  // Space takes a photo while live, unless typing in a field.
  const shootRef = useRef(shoot);
  shootRef.current = shoot;
  useEffect(() => {
    if (!running) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.code !== "Space" || e.repeat || (e.target as HTMLElement).closest("input, select")) return;
      e.preventDefault();
      void shootRef.current();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [running]);

  /** Checks each file parses, then keeps a copy in the app's looks folder. */
  async function addLooks(files: FileList | null) {
    const failed: string[] = [];
    for (const file of Array.from(files ?? [])) {
      try {
        const text = await file.text();
        parseCube(text, file.name);
        await camera.saveLut(file.name, text);
      } catch (e) {
        failed.push(`${file.name}: ${e instanceof Error ? e.message : e}`);
      }
    }
    setLookError(failed.join("\n"));
    setLooks(await camera.listLuts());
  }

  return (
    <main className="app">
      <canvas ref={canvas} className="view" />
      <div className="bar">
        {running ? (
          <>
            <button
              className="shoot"
              onClick={() => void shoot()}
              disabled={shot === "busy" || status?.state !== "live"}
            >
              {shot === "done" ? "✓ Shot" : "Shoot"}
            </button>
            {shot !== "idle" && shot !== "busy" && shot !== "done" && <p className="error">{shot}</p>}
            <Exposure live={status?.state === "live"} />
            <div className="row">
              <span className="status">{label(status)}</span>
              <button className="secondary" onClick={() => setRunning(false)}>
                Stop
              </button>
            </div>
          </>
        ) : (
          <form
            className="row"
            onSubmit={(e) => {
              e.preventDefault();
              remember(ADDRESS_KEY, address);
              setRunning(true);
            }}
          >
            <input
              value={address}
              onChange={(e) => setAddress(e.target.value)}
              placeholder="Camera address, e.g. 192.168.1.2:8080"
              inputMode="url"
            />
            <button type="submit" disabled={!address.trim()}>
              Start
            </button>
          </form>
        )}

        <label className="field">
          Look
          <select value={lookName} onChange={(e) => setLookName(e.target.value)}>
            <option value="">None</option>
            {looks.map((name) => (
              <option key={name} value={name}>
                {name.replace(/\.cube$/i, "")}
              </option>
            ))}
          </select>
        </label>
        <label className="link">
          Add looks…
          <input
            type="file"
            accept=".cube"
            multiple
            hidden
            onChange={(e) => {
              void addLooks(e.target.files);
              e.target.value = "";
            }}
          />
        </label>
        {lookError && <p className="error">{lookError}</p>}

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
        <label className="toggle">
          <input
            type="checkbox"
            checked={cameraDisplayOn}
            onChange={(e) => setCameraDisplayOn(e.target.checked)}
          />
          Camera screen
        </label>
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
    // Only a convenience: the value is chosen again next time.
  }
}

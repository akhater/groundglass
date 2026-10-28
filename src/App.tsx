import { useEffect, useRef, useState } from "react";
import * as camera from "./camera";
import { Connection } from "./Connection";
import { parseCube, type Cube } from "./cube";
import { FocusButton, SETTINGS, useExposure } from "./Exposure";
import { runLiveView, type Status } from "./liveView";
import { Strip } from "./Strip";

const ADDRESS_KEY = "groundglass.address";
const LOOK_KEY = "groundglass.look";
const ZEBRA_LEVELS = [70, 80, 85, 90, 95, 98, 100];

/** The strip of choices open over the picture. */
type Picker = camera.ExposureSetting | "look" | "zebra";

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
  const [picker, setPicker] = useState<Picker | null>(null);
  const [connectionOpen, setConnectionOpen] = useState(false);
  /** Messages hidden with a tap, shown again when they change. */
  const [dismissed, setDismissed] = useState("");
  // The loop reads these on every frame, so changing them needs no restart.
  const zebra = useRef<number | null>(null);
  zebra.current = zebraOn ? zebraPercent : null;
  const look = useRef<Cube | null>(null);

  const live = running && status?.state === "live";
  const exposure = useExposure(live);

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

  const closeAll = () => {
    setPicker(null);
    setConnectionOpen(false);
  };
  const toggle = (next: Picker) => {
    setConnectionOpen(false);
    setPicker((open) => (open === next ? null : next));
  };

  // Space takes a photo while live, unless typing in a field; Escape closes what is open.
  const onKey = useRef((_: KeyboardEvent) => {});
  onKey.current = (e) => {
    if (e.code === "Escape") closeAll();
    if (e.code !== "Space" || !live || e.repeat || (e.target as HTMLElement).closest("input")) return;
    e.preventDefault();
    void shoot();
  };
  useEffect(() => {
    const listener = (e: KeyboardEvent) => onKey.current(e);
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, []);

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

  function strip() {
    const close = () => setPicker(null);
    if (picker === "look") {
      return (
        <Strip
          label="Look"
          choices={[{ value: "", label: "None" }, ...looks.map((name) => ({ value: name, label: lookLabel(name) }))]}
          value={lookName}
          onPick={setLookName}
          onClose={close}
          extra={
            <label className="add">
              + Add
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
          }
        />
      );
    }
    if (picker === "zebra") {
      return (
        <Strip
          label="Zebra"
          choices={[{ value: "off", label: "Off" }, ...ZEBRA_LEVELS.map((n) => ({ value: String(n), label: `${n}%` }))]}
          value={zebraOn ? String(zebraPercent) : "off"}
          onPick={(value) => {
            setZebraOn(value !== "off");
            if (value !== "off") setZebraPercent(Number(value));
          }}
          onClose={close}
        />
      );
    }
    const state = picker && exposure.states[picker];
    if (!picker || !state || state.choices.length === 0) return null;
    const values = state.choices.includes(state.value) ? state.choices : [state.value, ...state.choices];
    return (
      <Strip
        label={SETTINGS.find((s) => s.key === picker)!.label}
        choices={values.map((value) => ({ value, label: value }))}
        value={state.value}
        onPick={(value) => void exposure.change(picker, value)}
        onClose={close}
      />
    );
  }

  const shotError = shot !== "idle" && shot !== "busy" && shot !== "done" ? shot : "";
  const errors = [running && status?.state === "lost" ? status.message : "", shotError, exposure.error, lookError]
    .filter(Boolean)
    .join("\n");

  return (
    <main className="app">
      <section
        className="stage"
        onClick={(e) => {
          // A tap on the picture closes whatever is open over it.
          if (e.target === e.currentTarget || e.target === canvas.current) closeAll();
        }}
      >
        <canvas ref={canvas} className="view" />
        {shot === "done" && <div className="flash" />}

        {running && (
          <button
            className={`status ${status?.state ?? "connecting"}`}
            aria-expanded={connectionOpen}
            onClick={() => {
              setPicker(null);
              setConnectionOpen((open) => !open);
            }}
          >
            {shortLabel(status)}
          </button>
        )}
        {errors && errors !== dismissed && (
          <button className="errors" onClick={() => setDismissed(errors)} title="Tap to hide">
            {errors}
          </button>
        )}

        {(!running || connectionOpen) && (
          <Connection
            running={running}
            address={address}
            onAddress={setAddress}
            cameraDisplayOn={cameraDisplayOn}
            onCameraDisplay={setCameraDisplayOn}
            status={label(status)}
            onStart={() => {
              remember(ADDRESS_KEY, address);
              setStatus(null);
              setRunning(true);
            }}
            onStop={() => {
              closeAll();
              setRunning(false);
            }}
          />
        )}
        {strip()}
      </section>

      <nav className="rail">
        <div className="chips">
          {SETTINGS.map(({ key, label }) => {
            const state = exposure.states[key];
            return (
              <Chip
                key={key}
                kind="exposure"
                label={label}
                value={state?.value ?? "–"}
                open={picker === key}
                disabled={!state || state.choices.length === 0}
                onClick={() => toggle(key)}
              />
            );
          })}
          <Chip
            kind="look"
            label="Look"
            value={lookName ? lookLabel(lookName) : "None"}
            open={picker === "look"}
            onClick={() => toggle("look")}
          />
          <Chip
            kind="zebra"
            label="Zebra"
            value={zebraOn ? `${zebraPercent}%` : "Off"}
            open={picker === "zebra"}
            onClick={() => toggle("zebra")}
          />
        </div>
        <div className="actions">
          <FocusButton focus={exposure.focus} disabled={!live} />
          <button
            className={`shoot ${shot === "busy" || shot === "done" ? shot : ""}`}
            onClick={() => void shoot()}
            disabled={shot === "busy" || !live}
            aria-label="Shoot"
          />
        </div>
      </nav>
    </main>
  );
}

type ChipProps = {
  kind: string;
  label: string;
  value: string;
  open: boolean;
  disabled?: boolean;
  onClick: () => void;
};

/** A setting's name and value; tapping it opens its strip of choices. */
function Chip({ kind, label, value, open, disabled, onClick }: ChipProps) {
  return (
    <button className={`chip ${kind}`} aria-expanded={open} disabled={disabled} onClick={onClick}>
      <span>{label}</span>
      <strong>{value}</strong>
    </button>
  );
}

const lookLabel = (name: string) => name.replace(/\.cube$/i, "");

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

/** The status on the picture: short, the full one is in the connection panel. */
function shortLabel(status: Status | null): string {
  switch (status?.state) {
    case "live":
      return `${status.fps} fps`;
    case "lost":
      return "Retrying…";
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

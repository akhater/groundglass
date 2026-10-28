// The screen: the live picture with what opens over it, and the controls beside it.
import { useEffect, useRef, useState } from "react";
import type * as camera from "./camera";
import { Connection, StatusBadge } from "./Connection";
import { FocusButton, SETTINGS, useExposure } from "./Exposure";
import { ShootButton, useShoot } from "./ShootButton";
import { remember, remembered } from "./storage";
import { Strip } from "./Strip";
import { useLiveView } from "./useLiveView";
import { lookLabel, useLooks } from "./useLooks";

const ADDRESS_KEY = "groundglass.address";
const ZEBRA_LEVELS = [70, 80, 85, 90, 95, 98, 100];

/** The strip of choices open over the picture. */
type Picker = camera.ExposureSetting | "look" | "zebra";

export default function App() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [address, setAddress] = useState(() => remembered(ADDRESS_KEY) ?? "");
  // On by default: with the camera's screen off, its own controls are locked too.
  const [cameraDisplayOn, setCameraDisplayOn] = useState(true);
  const [running, setRunning] = useState(false);
  const [zebraOn, setZebraOn] = useState(true);
  const [zebraPercent, setZebraPercent] = useState(98);
  const [picker, setPicker] = useState<Picker | null>(null);
  const [connectionOpen, setConnectionOpen] = useState(false);
  /** Messages hidden with a tap, shown again when they change. */
  const [dismissed, setDismissed] = useState("");
  // The frame loop reads this on every frame, so changing it needs no restart.
  const zebra = useRef<number | null>(null);
  zebra.current = zebraOn ? zebraPercent : null;

  const looks = useLooks();
  const status = useLiveView({ canvas, running, address, cameraDisplayOn, zebraPercent: zebra, look: looks.cube });
  const live = status?.state === "live";
  const exposure = useExposure(live);
  const shot = useShoot(live);

  const closeAll = () => {
    setPicker(null);
    setConnectionOpen(false);
  };
  const toggle = (next: Picker) => {
    setConnectionOpen(false);
    setPicker((open) => (open === next ? null : next));
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.code === "Escape") closeAll();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  function strip() {
    const close = () => setPicker(null);
    if (picker === "look") {
      return (
        <Strip
          label="Look"
          choices={[{ value: "", label: "None" }, ...looks.names.map((name) => ({ value: name, label: lookLabel(name) }))]}
          value={looks.chosen}
          onPick={looks.choose}
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
                  void looks.add(e.target.files);
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

  const errors = [status?.state === "lost" ? status.message : "", shot.error, exposure.error, looks.error]
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
        {shot.state === "done" && <div className="flash" />}

        {running && (
          <StatusBadge
            status={status}
            open={connectionOpen}
            onToggle={() => {
              setPicker(null);
              setConnectionOpen((open) => !open);
            }}
          />
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
            status={status}
            onStart={() => {
              remember(ADDRESS_KEY, address);
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
            value={looks.chosen ? lookLabel(looks.chosen) : "None"}
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
          <ShootButton state={shot.state} disabled={!live} onShoot={() => void shot.shoot()} />
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

// The screen: the live picture with what opens over it, and the controls beside it.
import { useEffect, useRef, useState } from "react";
import type * as camera from "./camera";
import { Connection, StatusBadge } from "./Connection";
import { FocusButton, SETTINGS, useExposure } from "./Exposure";
import { AfFrames } from "./AfFrames";
import { Composition, GUIDES, TURNING, type Guide } from "./Composition";
import { Crop, cropBox, RATIOS, type Ratio } from "./Crop";
import { Level } from "./Level";
import { pictureBox } from "./picture";
import { ShootButton, useShoot } from "./ShootButton";
import { remembered, rememberedList, rememberFirst } from "./storage";
import type { Peaking } from "./render";
import { Strip } from "./Strip";
import { useLiveDetails } from "./useLiveDetails";
import { useLiveView } from "./useLiveView";
import { useTapFocus } from "./useTapFocus";
import { lookLabel, useLooks } from "./useLooks";

const ADDRESSES_KEY = "groundglass.addresses";
/** Before the list, only the last address was kept, under this key. */
const OLD_ADDRESS_KEY = "groundglass.address";
const ADDRESSES_KEPT = 10;
const ZEBRA_LEVELS = [70, 80, 85, 90, 95, 98, 100];
const PEAKING_CHOICES: { value: Peaking | "off"; label: string }[] = [
  { value: "off", label: "Off" },
  { value: "low", label: "Low" },
  { value: "mid", label: "Mid" },
  { value: "high", label: "High" },
];

/** The strip of choices open over the picture. */
type Picker = camera.ExposureSetting | "look" | "zebra" | "peaking" | "grid" | "crop";

export default function App() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [recent, setRecent] = useState(() => {
    const list = rememberedList(ADDRESSES_KEY);
    const old = remembered(OLD_ADDRESS_KEY);
    return list.length === 0 && old ? [old] : list;
  });
  const [address, setAddress] = useState(() => recent[0] ?? "");
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
  const [peakingChoice, setPeakingChoice] = useState<Peaking | "off">("off");
  const peaking = useRef<Peaking | null>(null);
  peaking.current = peakingChoice === "off" ? null : peakingChoice;

  const looks = useLooks();
  const status = useLiveView({ canvas, running, address, cameraDisplayOn, zebraPercent: zebra, look: looks.cube, peaking });
  const live = status?.state === "live";
  const exposure = useExposure(live);
  const shot = useShoot(live);
  const [levelOn, setLevelOn] = useState(false);
  const [guide, setGuide] = useState<Guide | "off">("off");
  /** Quarter turns of the triangle or spiral, 0..3. */
  const [guideRotation, setGuideRotation] = useState(0);
  const [crop, setCrop] = useState<Ratio | "off">("off");
  const details = useLiveDetails(live);
  const tapFocus = useTapFocus(canvas);

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
    if (picker === "grid") {
      return (
        <Strip
          label="Grid"
          choices={[{ value: "off", label: "Off" }, ...GUIDES]}
          value={guide}
          onPick={(value) => setGuide(value as Guide | "off")}
          onClose={close}
          extra={
            guide !== "off" &&
            TURNING.includes(guide) && (
              <button className="add" onClick={() => setGuideRotation((turns) => (turns + 1) % 4)}>
                Rotate
              </button>
            )
          }
        />
      );
    }
    if (picker === "crop") {
      return (
        <Strip
          label="Crop"
          choices={[{ value: "off", label: "Off" }, ...RATIOS.map((ratio) => ({ value: ratio, label: ratio }))]}
          value={crop}
          onPick={(value) => setCrop(value as Ratio | "off")}
          onClose={close}
        />
      );
    }
    if (picker === "peaking") {
      return (
        <Strip
          label="Peaking"
          choices={PEAKING_CHOICES}
          value={peakingChoice}
          onPick={(value) => setPeakingChoice(value as Peaking | "off")}
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

  const errors = [status?.state === "lost" ? status.message : "", shot.error, exposure.error, looks.error, tapFocus.error]
    .filter(Boolean)
    .join("\n");

  // Where the picture sits on screen, for what is drawn over it.
  const picture = canvas.current && pictureBox(canvas.current);

  return (
    <main className="app">
      <section
        className="stage"
        onClick={(e) => {
          if (e.target !== e.currentTarget && e.target !== canvas.current) return;
          // A tap on the picture closes whatever is open over it; with nothing
          // open, it moves the camera's AF point there.
          if (picker || connectionOpen) closeAll();
          else if (live) void tapFocus.focusAt(e);
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
        {details && <AfFrames frames={details.afFrames} box={picture} />}
        {live && crop !== "off" && picture && <Crop box={picture} ratio={crop} />}
        {live && guide !== "off" && (
          // Inside the crop when there is one: the guides are for the final frame.
          <Composition guide={guide} rotation={guideRotation} box={picture && (crop === "off" ? picture : cropBox(picture, crop))} />
        )}
        {live && levelOn && <Level level={details?.level ?? null} />}
        {live && (
          <div className="stage-tools">
            <button aria-pressed={crop !== "off"} aria-expanded={picker === "crop"} onClick={() => toggle("crop")}>
              Crop
            </button>
            <button aria-pressed={guide !== "off"} aria-expanded={picker === "grid"} onClick={() => toggle("grid")}>
              Grid
            </button>
            <button aria-pressed={levelOn} onClick={() => setLevelOn((on) => !on)}>
              Level
            </button>
          </div>
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
            recent={recent}
            cameraDisplayOn={cameraDisplayOn}
            onCameraDisplay={setCameraDisplayOn}
            status={status}
            onStart={() => {
              const used = address.trim();
              setAddress(used);
              setRecent(rememberFirst(ADDRESSES_KEY, recent, used, ADDRESSES_KEPT));
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
          <Chip
            kind="peaking"
            label="Peaking"
            value={PEAKING_CHOICES.find((c) => c.value === peakingChoice)!.label}
            open={picker === "peaking"}
            onClick={() => toggle("peaking")}
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

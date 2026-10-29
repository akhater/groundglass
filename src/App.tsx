// The screen: the live picture, the aids drawn over it in one band, the camera
// in the other, and the choices that open beside them.
import { useEffect, useRef, useState } from "react";
import type * as camera from "./camera";
import { Connection, StatusBadge } from "./Connection";
import { Setting, Tool } from "./Controls";
import { FocusButton, SETTINGS, useExposure } from "./Exposure";
import { AfFrames } from "./AfFrames";
import { Composition, GuideArt, GUIDES, TURNING, type Guide } from "./Composition";
import { Crop, CropArt, cropBox, RATIOS, type Ratio } from "./Crop";
import { EttrButton, useEttr } from "./Ettr";
import { Histogram } from "./Histogram";
import { Icon } from "./icons";
import { Level } from "./Level";
import { MagnifyTools, useMagnify } from "./Magnify";
import { pictureBox } from "./picture";
import { ShootButton, useShoot } from "./ShootButton";
import { remembered, rememberedList, rememberFirst } from "./storage";
import type { Peaking } from "./render";
import { Scale } from "./Scale";
import { Tiles } from "./Tiles";
import { useBattery } from "./useBattery";
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
/** The camera's settings in the order its own screen shows them. */
const CAMERA_ORDER: camera.ExposureSetting[] = ["tv", "av", "iso"];

/** The choices open beside the picture. */
type Picker = camera.ExposureSetting | "look" | "zebra" | "peaking" | "grid" | "crop";
type Side = "assists" | "camera";

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
  const zoom = useRef<camera.Zoom>(1);
  const status = useLiveView({ canvas, running, address, cameraDisplayOn, zebraPercent: zebra, look: looks.cube, peaking, zoom });
  const live = status?.state === "live";
  const magnify = useMagnify(live, zoom);
  const exposure = useExposure(live);
  const ettr = useEttr(live, exposure.states, exposure.refresh);
  const shot = useShoot(live);
  const [levelOn, setLevelOn] = useState(false);
  const [histogramOn, setHistogramOn] = useState(false);
  const [guide, setGuide] = useState<Guide | "off">("off");
  /** Quarter turns of the triangle or spiral, 0..3. */
  const [guideRotation, setGuideRotation] = useState(0);
  const [crop, setCrop] = useState<Ratio | "off">("off");
  const details = useLiveDetails(live);
  const battery = useBattery(live);
  const tapFocus = useTapFocus(canvas);
  /** Large screens only, where the bands sit over the picture: a band tucked away. */
  const [folded, setFolded] = useState({ assists: false, camera: false });
  const fold = (side: Side) => setFolded((now) => ({ ...now, [side]: !now[side] }));

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

  function choices() {
    const close = () => setPicker(null);
    if (picker === "look") {
      return (
        <Tiles
          side="camera"
          label="Look"
          tiles={[{ value: "", label: "None" }, ...looks.names.map((name) => ({ value: name, label: lookLabel(name) }))]}
          value={looks.chosen}
          onPick={looks.choose}
          onClose={close}
          first={
            <label className="tile add">
              <Icon name="plus" />
              <span>Add .cube</span>
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
        <Tiles
          side="assists"
          label="Zebra"
          tiles={[{ value: "off", label: "Off" }, ...ZEBRA_LEVELS.map((n) => ({ value: String(n), label: `${n}%` }))]}
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
      const turns = guide !== "off" && TURNING.includes(guide);
      const rotate = () => setGuideRotation((turns) => (turns + 1) % 4);
      return (
        <Tiles
          side="assists"
          label="Guides"
          tiles={[
            { value: "off", label: "Off", art: <NoneArt /> },
            ...GUIDES.map((g) => ({ ...g, art: <GuideArt guide={g.value} rotation={g.value === guide ? guideRotation : 0} /> })),
          ]}
          value={guide}
          // Tapping the chosen triangle or spiral again turns it, as Rotate does.
          onPick={(value) => (value === guide && turns ? rotate() : setGuide(value as Guide | "off"))}
          onClose={close}
          action={
            <button className="action" disabled={!turns} onClick={rotate}>
              <Icon name="rotate" />
              Rotate
            </button>
          }
        />
      );
    }
    if (picker === "crop") {
      return (
        <Tiles
          side="assists"
          label="Crop"
          tiles={[
            { value: "off", label: "Off", art: <NoneArt /> },
            ...RATIOS.map((ratio) => ({ value: ratio, label: ratio, art: <CropArt ratio={ratio} /> })),
          ]}
          value={crop}
          onPick={(value) => setCrop(value as Ratio | "off")}
          onClose={close}
        />
      );
    }
    if (picker === "peaking") {
      return (
        <Tiles
          side="assists"
          label="Peaking"
          tiles={PEAKING_CHOICES}
          value={peakingChoice}
          onPick={(value) => setPeakingChoice(value as Peaking | "off")}
          onClose={close}
        />
      );
    }
    const state = picker && exposure.states[picker];
    if (!picker || !state || state.choices.length === 0) return null;
    const pinned = ettr.pinned.includes(picker);
    return (
      <Scale
        label={SETTINGS.find((s) => s.key === picker)!.label}
        values={state.choices.includes(state.value) ? state.choices : [state.value, ...state.choices]}
        value={state.value}
        onPick={(value) => void exposure.change(picker, value)}
        onClose={close}
        action={
          // ETTR keeps a pinned setting as it is; two at most, so one is left to move.
          <button className="action" aria-pressed={pinned} disabled={!ettr.canPin(picker)} onClick={() => ettr.pin(picker)}>
            <Icon name="pin" />
            {pinned ? "Pinned" : "Pin"}
          </button>
        }
      />
    );
  }

  const errors = [
    status?.state === "lost" ? status.message : status?.state === "connecting" ? (status.hint ?? "") : "",
    shot.error,
    exposure.error,
    ettr.error,
    looks.error,
    tapFocus.error,
    magnify.error,
  ]
    .filter(Boolean)
    .join("\n");

  // Where the picture sits on screen, for what is drawn over it.
  const picture = canvas.current && pictureBox(canvas.current);
  const roll = details?.level ? `${details.level.roll.toFixed(1)}°` : "On";

  return (
    <main className={`app${folded.assists ? " fold-assists" : ""}${folded.camera ? " fold-camera" : ""}`}>
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
        {details && <AfFrames frames={details.afFrames} box={picture} />}
        {live && crop !== "off" && picture && <Crop box={picture} ratio={crop} />}
        {live && guide !== "off" && (
          // Inside the crop when there is one: the guides are for the final frame.
          <Composition guide={guide} rotation={guideRotation} box={picture && (crop === "off" ? picture : cropBox(picture, crop))} />
        )}
        {live && levelOn && <Level level={details?.level ?? null} box={picture} />}
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
      </section>

      {/* Left thumb: the status, and the aids drawn over the picture. */}
      <aside className="band assists">
        {running && (
          <StatusBadge
            status={status}
            battery={battery}
            open={connectionOpen}
            onToggle={() => {
              setPicker(null);
              setConnectionOpen((open) => !open);
            }}
          />
        )}
        <div className="tools">
          <Tool
            icon="zebra"
            label="Zebra"
            value={zebraOn ? `${zebraPercent}%` : "Off"}
            on={zebraOn}
            open={picker === "zebra"}
            onClick={() => toggle("zebra")}
          />
          <Tool
            icon="peaking"
            label="Peaking"
            value={PEAKING_CHOICES.find((c) => c.value === peakingChoice)!.label}
            on={peakingChoice !== "off"}
            open={picker === "peaking"}
            onClick={() => toggle("peaking")}
          />
          <Tool
            icon="guides"
            label="Guides"
            value={GUIDES.find((g) => g.value === guide)?.label ?? "Off"}
            on={guide !== "off"}
            open={picker === "grid"}
            onClick={() => toggle("grid")}
          />
          <Tool
            icon="crop"
            label="Crop"
            value={crop === "off" ? "Off" : crop}
            on={crop !== "off"}
            open={picker === "crop"}
            onClick={() => toggle("crop")}
          />
          <Tool
            icon="level"
            label="Level"
            value={levelOn ? roll : "Off"}
            on={levelOn}
            disabled={!live}
            onClick={() => setLevelOn((on) => !on)}
          />
          <Tool
            icon="histogram"
            label="Histogram"
            value={histogramOn ? "On" : "Off"}
            on={histogramOn}
            disabled={!live}
            onClick={() => setHistogramOn((on) => !on)}
          />
          <MagnifyTools camera={status?.state === "live" ? status.camera : null} magnify={magnify} />
        </div>
        <FoldButton side="assists" folded={folded.assists} onFold={fold} />
      </aside>

      {live && histogramOn && <Histogram counts={details?.histogram ?? null} box={picture} />}
      {choices()}

      {/* Right thumb: the camera. */}
      <aside className="band camera">
        <div className="settings">
          {CAMERA_ORDER.map((key) => {
            const state = exposure.states[key];
            return (
              <Setting
                key={key}
                label={SETTINGS.find((s) => s.key === key)!.label}
                value={state?.value ?? "–"}
                open={picker === key}
                disabled={!state || state.choices.length === 0}
                pinned={ettr.pinned.includes(key)}
                flash={ettr.flash.includes(key)}
                onClick={() => toggle(key)}
              />
            );
          })}
          <Setting
            label="Look"
            value={looks.chosen ? lookLabel(looks.chosen) : "None"}
            open={picker === "look"}
            onClick={() => toggle("look")}
          />
        </div>
        <div className="actions">
          <EttrButton ettr={ettr} />
          <FocusButton focus={exposure.focus} disabled={!live} />
          <ShootButton state={shot.state} disabled={!live} onShoot={() => void shot.shoot()} />
        </div>
        <FoldButton side="camera" folded={folded.camera} onFold={fold} />
      </aside>
    </main>
  );
}

/** On a large screen the bands sit over the picture's edges: this tucks one away, or brings it back. */
function FoldButton({ side, folded, onFold }: { side: Side; folded: boolean; onFold: (side: Side) => void }) {
  const pointsRight = (side === "assists") === folded;
  return (
    <button className="fold" aria-label={folded ? "Show the controls" : "Hide the controls"} onClick={() => onFold(side)}>
      <Icon name={pointsRight ? "right" : "left"} />
    </button>
  );
}

/** The tile drawing for Off: an empty frame. */
function NoneArt() {
  return (
    <svg className="art" viewBox="-1 -1 32 22" aria-hidden="true">
      <rect className="faint" width={30} height={20} rx={1.5} />
    </svg>
  );
}

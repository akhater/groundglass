// Expose to the right: one tap sets the brightest exposure that clips nothing,
// moving only the settings not pinned. The camera does the rounds (ettr_run.rs);
// this keeps the pins and shows how it ended:
// a setting at the end of its range flashes, as does the button when live view
// didn't follow (exposure simulation off).
import { useState } from "react";
import * as camera from "./camera";
import { remember, remembered } from "./storage";

const PINNED_KEY = "groundglass.ettrPinned";
/** Two pins at most: ETTR needs one setting to move. */
const MOST_PINNED = 2;
/** Until changed: aperture (depth of field) and shutter (motion) stay, ISO moves. */
const FIRST_PINNED: camera.ExposureSetting[] = ["av", "tv"];
/** How long a flash shows. */
const FLASH_MS = 1600;

type States = Partial<Record<camera.ExposureSetting, camera.SettingState>>;

function rememberedPins(): camera.ExposureSetting[] {
  try {
    const list: unknown = JSON.parse(remembered(PINNED_KEY) ?? "null");
    if (!Array.isArray(list)) return FIRST_PINNED;
    return list.filter((key) => key === "tv" || key === "av" || key === "iso").slice(0, MOST_PINNED);
  } catch {
    return FIRST_PINNED;
  }
}

export function useEttr(live: boolean, states: States, refresh: () => Promise<void>) {
  const [pinned, setPinned] = useState(rememberedPins);
  const [running, setRunning] = useState(false);
  /** What flashes after a run: settings at the end of their range, or the button. */
  const [flash, setFlash] = useState<(camera.ExposureSetting | "button")[]>([]);
  const [error, setError] = useState("");

  // As ettr_run.rs asks: the free settings settable and not on auto, none pinned on auto (Auto ISO).
  const usable = (["tv", "av", "iso"] as const).every((key) => {
    const state = states[key];
    if (!state || state.value === "auto") return false;
    return pinned.includes(key) || state.choices.length > 0;
  });

  function pin(key: camera.ExposureSetting) {
    const next = pinned.includes(key) ? pinned.filter((k) => k !== key) : [...pinned, key];
    if (next.length > MOST_PINNED) return;
    setPinned(next);
    remember(PINNED_KEY, JSON.stringify(next));
  }

  async function run() {
    if (running) return;
    setRunning(true);
    setError("");
    try {
      const outcome = await camera.exposeToTheRight(pinned);
      if (outcome.kind === "Limited") show(outcome.detail);
      if (outcome.kind === "NoResponse" || outcome.kind === "Unavailable") show(["button"]);
    } catch (e) {
      setError(camera.describe(e));
    }
    setRunning(false);
    void refresh();
  }

  function show(what: (camera.ExposureSetting | "button")[]) {
    setFlash(what);
    window.setTimeout(() => setFlash([]), FLASH_MS);
  }

  return {
    pinned,
    canPin: (key: camera.ExposureSetting) => pinned.includes(key) || pinned.length < MOST_PINNED,
    pin,
    running,
    flash,
    error,
    run,
    disabled: !live || !usable,
  };
}

export function EttrButton({ ettr }: { ettr: ReturnType<typeof useEttr> }) {
  const className = `ettr${ettr.running ? " running" : ""}${ettr.flash.includes("button") ? " limit" : ""}`;
  return (
    <button
      className={className}
      disabled={ettr.disabled}
      onClick={() => void ettr.run()}
      aria-label="Expose to the right"
    >
      ETTR
    </button>
  );
}

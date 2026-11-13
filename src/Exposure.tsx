// ISO, aperture and shutter speed, plus back-button focus, while live view runs.
import { useEffect, useRef, useState } from "react";
import * as camera from "./camera";

export const SETTINGS: { key: camera.ExposureSetting; label: string }[] = [
  { key: "iso", label: "ISO" },
  { key: "av", label: "Aperture" },
  { key: "tv", label: "Shutter" },
];

/** Re-read this often, so the camera's own changes show up: its dials, and what it meters in Av, Tv and P. */
const REFRESH_MS = 1000;

type States = Partial<Record<camera.ExposureSetting, camera.SettingState>>;

/** The camera sets it in this shooting mode (the shutter in Av): it shows the camera's value, not openable. */
export const setByCamera = (state: camera.SettingState | undefined) => !!state && state.choices.length === 0;

/** Why a setting can't be changed now, in one line. */
export const setByCameraNote = (mode: string | null) => `Set by the camera in ${mode ?? "this mode"}`;

export function useExposure(live: boolean) {
  const [states, setStates] = useState<States>({});
  const [mode, setMode] = useState<string | null>(null);
  const [error, setError] = useState("");
  /** Reads are numbered: one that started before what is shown would show older values, and is dropped. */
  const reads = useRef(0);
  const shown = useRef(0);

  async function refresh() {
    const read = ++reads.current;
    const next: States = {};
    for (const { key } of SETTINGS) {
      try {
        next[key] = await camera.exposureSetting(key);
      } catch {
        // Not every mode or body has every setting: leave it out.
      }
    }
    const nextMode = await camera.shootingMode().catch(() => null);
    if (read < shown.current) return;
    shown.current = read;
    setStates(next);
    setMode(nextMode);
  }

  useEffect(() => {
    if (!live) {
      setStates({});
      return;
    }
    let running = true;
    void (async () => {
      // One read at a time: a slow connection spaces them out instead of piling them up.
      while (running) {
        await refresh();
        await new Promise((resolve) => setTimeout(resolve, REFRESH_MS));
      }
    })();
    return () => {
      running = false;
      shown.current = ++reads.current;
    };
  }, [live]);

  async function change(key: camera.ExposureSetting, value: string) {
    // Show the new value at once; the refresh corrects it if the camera refused.
    shown.current = ++reads.current;
    setStates((current) => {
      const state = current[key];
      return state ? { ...current, [key]: { ...state, value } } : current;
    });
    try {
      await camera.setExposureSetting(key, value);
      setError("");
    } catch (e) {
      setError(camera.describe(e));
    }
    void refresh();
  }

  const focus = (held: boolean) => void camera.autofocus(held).catch((e) => setError(camera.describe(e)));

  return { states, mode, error, change, focus, refresh };
}

/** Back-button focus: the camera focuses for as long as this is held. */
export function FocusButton({ focus, disabled }: { focus: (held: boolean) => void; disabled: boolean }) {
  const [held, setHeld] = useState(false);
  const heldNow = useRef(false);
  const hold = (on: boolean) => {
    if (heldNow.current === on) return;
    heldNow.current = on;
    setHeld(on);
    focus(on);
  };

  return (
    <button
      className={held ? "af held" : "af"}
      disabled={disabled}
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        hold(true);
      }}
      onPointerUp={() => hold(false)}
      onPointerCancel={() => hold(false)}
      onLostPointerCapture={() => hold(false)}
      onContextMenu={(e) => e.preventDefault()}
    >
      AF-ON
      <small>hold</small>
    </button>
  );
}

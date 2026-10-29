// ISO, aperture and shutter speed, plus back-button focus, while live view runs.
import { useEffect, useRef, useState } from "react";
import * as camera from "./camera";

export const SETTINGS: { key: camera.ExposureSetting; label: string }[] = [
  { key: "iso", label: "ISO" },
  { key: "av", label: "Aperture" },
  { key: "tv", label: "Shutter" },
];

/** Re-read this often, so changes made on the camera's dials show up. */
const REFRESH_MS = 4000;

type States = Partial<Record<camera.ExposureSetting, camera.SettingState>>;

/** The camera sets it in this shooting mode (the shutter in Av): it shows the camera's value, not openable. */
export const setByCamera = (state: camera.SettingState | undefined) => !!state && state.choices.length === 0;

/** Why a setting can't be changed now, in one line. */
export const setByCameraNote = (mode: string | null) => `Set by the camera in ${mode ?? "this mode"}`;

export function useExposure(live: boolean) {
  const [states, setStates] = useState<States>({});
  const [mode, setMode] = useState<string | null>(null);
  const [error, setError] = useState("");

  async function refresh() {
    const next: States = {};
    for (const { key } of SETTINGS) {
      try {
        next[key] = await camera.exposureSetting(key);
      } catch {
        // Not every mode or body has every setting: leave it out.
      }
    }
    setStates(next);
    setMode(await camera.shootingMode().catch(() => null));
  }

  useEffect(() => {
    if (!live) {
      setStates({});
      return;
    }
    void refresh();
    const timer = setInterval(() => void refresh(), REFRESH_MS);
    return () => clearInterval(timer);
  }, [live]);

  async function change(key: camera.ExposureSetting, value: string) {
    // Show the new value at once; the refresh corrects it if the camera refused.
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

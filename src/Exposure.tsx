// ISO, aperture and shutter speed, plus back-button focus, while live view runs.
import { useEffect, useState } from "react";
import * as camera from "./camera";

const SETTINGS: { key: camera.ExposureSetting; label: string }[] = [
  { key: "iso", label: "ISO" },
  { key: "av", label: "Aperture" },
  { key: "tv", label: "Shutter" },
];

/** Re-read this often, so changes made on the camera's dials show up. */
const REFRESH_MS = 4000;

type States = Partial<Record<camera.ExposureSetting, camera.SettingState>>;

export function Exposure({ live }: { live: boolean }) {
  const [states, setStates] = useState<States>({});
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
  }

  useEffect(() => {
    if (!live) return;
    void refresh();
    const timer = setInterval(() => void refresh(), REFRESH_MS);
    return () => clearInterval(timer);
  }, [live]);

  async function change(key: camera.ExposureSetting, value: string) {
    try {
      await camera.setExposureSetting(key, value);
      setError("");
    } catch (e) {
      setError(camera.describe(e));
    }
    void refresh();
  }

  const focus = (held: boolean) => camera.autofocus(held).catch((e) => setError(camera.describe(e)));

  if (!live) return null;
  return (
    <>
      <div className="exposure">
        {SETTINGS.map(({ key, label }) => {
          const state = states[key];
          return (
            <label key={key} className="setting">
              <span>{label}</span>
              <select
                value={state?.value ?? ""}
                disabled={!state || state.choices.length === 0}
                onChange={(e) => void change(key, e.target.value)}
              >
                {state && !state.choices.includes(state.value) && <option value={state.value}>{state.value}</option>}
                {state?.choices.map((choice) => (
                  <option key={choice} value={choice}>
                    {choice}
                  </option>
                ))}
              </select>
            </label>
          );
        })}
      </div>
      <button
        className="secondary af"
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId);
          void focus(true);
        }}
        onPointerUp={() => void focus(false)}
        onPointerCancel={() => void focus(false)}
      >
        AF-ON (hold)
      </button>
      {error && <p className="error">{error}</p>}
    </>
  );
}

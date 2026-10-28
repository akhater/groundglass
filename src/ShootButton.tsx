// Taking a photo: the round shutter button, or Space on a keyboard.
import { useEffect, useRef, useState } from "react";
import * as camera from "./camera";

export type ShotState = "idle" | "busy" | "done";

/** How long "done" and a failure stay on screen. */
const SHOWN_MS = 2000;

export function useShoot(live: boolean) {
  const [state, setState] = useState<ShotState>("idle");
  const [error, setError] = useState("");
  const shown = useRef(0);

  async function shoot() {
    if (state === "busy") return;
    setState("busy");
    try {
      await camera.shoot();
      setState("done");
    } catch (e) {
      setState("idle");
      setError(camera.describe(e));
    }
    // A later shot restarts the clock rather than being cut short by this one's.
    clearTimeout(shown.current);
    shown.current = window.setTimeout(() => {
      setState("idle");
      setError("");
    }, SHOWN_MS);
  }

  // Space takes a photo while live, unless typing in a field.
  const onKey = useRef((_: KeyboardEvent) => {});
  onKey.current = (e) => {
    if (e.code !== "Space" || !live || e.repeat || (e.target as HTMLElement).closest("input")) return;
    e.preventDefault();
    void shoot();
  };
  useEffect(() => {
    const listener = (e: KeyboardEvent) => onKey.current(e);
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, []);

  return { state, error, shoot };
}

export function ShootButton({ state, disabled, onShoot }: { state: ShotState; disabled: boolean; onShoot: () => void }) {
  return (
    <button
      className={`shoot ${state}`}
      onClick={onShoot}
      disabled={disabled || state === "busy"}
      aria-label="Shoot"
    />
  );
}

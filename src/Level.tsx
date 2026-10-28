// A horizon line over the picture that turns with the camera, green when level.
import type * as camera from "./camera";

/** Within this many degrees counts as level, as on the camera's own level. */
const LEVEL_DEGREES = 0.5;

export function Level({ level }: { level: camera.Level | null }) {
  if (!level) {
    return (
      <div className="level">
        <span>Level…</span>
      </div>
    );
  }
  const flat = Math.abs(level.roll) <= LEVEL_DEGREES;
  return (
    <div className={flat ? "level flat" : "level"} aria-label="Camera level">
      <div className="horizon" style={{ transform: `rotate(${-level.roll}deg)` }} />
      <span>
        {level.roll.toFixed(1)}° · tilt {level.pitch.toFixed(1)}°
      </span>
    </div>
  );
}

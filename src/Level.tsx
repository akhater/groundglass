// The camera's level as two scales over the picture, as on a Hasselblad:
// roll along the bottom, tilt (pitch) up the side. A marker slides along each
// and turns green when within half a degree of level.
import type * as camera from "./camera";

/** Within this many degrees counts as level, as on the camera's own level. */
const LEVEL_DEGREES = 0.5;
/** Each scale runs from -RANGE to +RANGE degrees; beyond, the marker waits at the end. */
const RANGE = 10;

/** Where `degrees` sits along a scale, in percent from its start. */
const along = (degrees: number) => 50 + (Math.max(-RANGE, Math.min(RANGE, degrees)) / RANGE) * 50;

export function Level({ level }: { level: camera.Level | null }) {
  if (!level) {
    return (
      <div className="level">
        <span className="roll-value">Level…</span>
      </div>
    );
  }
  const flat = (degrees: number) => (Math.abs(degrees) <= LEVEL_DEGREES ? "flat" : "");
  return (
    <div className="level" aria-label="Camera level">
      <div className={`scale roll ${flat(level.roll)}`}>
        <i style={{ left: `${along(level.roll)}%` }} />
      </div>
      <span className="roll-value">{level.roll.toFixed(1)}°</span>
      <div className={`scale pitch ${flat(level.pitch)}`}>
        <i style={{ top: `${along(-level.pitch)}%` }} />
      </div>
      <span className="pitch-value">{level.pitch.toFixed(1)}°</span>
    </div>
  );
}

// The camera's luminance histogram, in the picture's bottom left corner (above
// a strip of choices when one is open there), dark to bright,
// in 64 bars so the end ones stay visible on a small screen. Its height is set
// by the tallest bar between the two ends, so a pile of pure black or pure white
// (clipping) reaches the top instead of flattening the rest.
import type { CSSProperties } from "react";
import type { Box } from "./picture";

const BARS = 64;

type Props = { counts: number[] | null; box: Box | null };

export function Histogram({ counts, box }: Props) {
  // Without a picture yet, styles.css falls back to the screen's corner.
  const corner = box && ({ "--picture-left": `${box.left}px`, "--picture-bottom": `${box.top + box.height}px` } as CSSProperties);
  if (!counts || counts.length < BARS) {
    return (
      <div className="histogram" style={corner ?? undefined}>
        Histogram…
      </div>
    );
  }
  const bars = Array.from({ length: BARS }, (_, bar) => {
    const from = Math.floor((bar * counts.length) / BARS);
    const to = Math.floor(((bar + 1) * counts.length) / BARS);
    return counts.slice(from, to).reduce((sum, count) => sum + count, 0);
  });
  const peak = Math.max(1, ...bars.slice(1, -1));
  const steps = bars.map((count, bar) => {
    const top = 100 - Math.min(1, count / peak) * 100;
    return `L${bar},${top} L${bar + 1},${top}`;
  });
  return (
    <div className="histogram" style={corner ?? undefined} aria-label="Histogram">
      <svg viewBox={`0 0 ${BARS} 100`} preserveAspectRatio="none">
        <path d={`M0,100 ${steps.join(" ")} L${BARS},100 Z`} />
      </svg>
    </div>
  );
}

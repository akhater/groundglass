// A crop frame over the picture, visual only: what falls outside the chosen
// aspect ratio is darkened. The photo itself is never cropped.
import type { Box } from "./picture";

export type Ratio = "1:1" | "5:4" | "4:3" | "16:9";

export const RATIOS: Ratio[] = ["1:1", "5:4", "4:3", "16:9"];

/** The largest box of `ratio` (width:height) centred in `box`. */
export function cropBox(box: Box, ratio: Ratio): Box {
  const [a, b] = ratio.split(":").map(Number);
  const width = Math.min(box.width, (box.height * a) / b);
  const height = Math.min(box.height, (box.width * b) / a);
  return {
    left: box.left + (box.width - width) / 2,
    top: box.top + (box.height - height) / 2,
    width,
    height,
  };
}

export function Crop({ box, ratio }: { box: Box; ratio: Ratio }) {
  const inner = cropBox({ ...box, left: 0, top: 0 }, ratio);
  return (
    <div className="crop" style={{ left: box.left, top: box.top, width: box.width, height: box.height }}>
      <div style={{ left: inner.left, top: inner.top, width: inner.width, height: inner.height }} />
    </div>
  );
}

/** A small drawing of `ratio` for its tile: the frame inside the picture's 3:2. */
export function CropArt({ ratio }: { ratio: Ratio }) {
  const inner = cropBox({ left: 0, top: 0, width: 30, height: 20 }, ratio);
  return (
    <svg className="art" viewBox="-1 -1 32 22" aria-hidden="true">
      <rect className="faint" width={30} height={20} rx={1.5} />
      <rect x={inner.left} y={inner.top} width={inner.width} height={inner.height} rx={1} />
    </svg>
  );
}

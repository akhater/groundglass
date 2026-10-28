// Composition guides over the picture, the set Lightroom's crop tool offers:
// thirds, the golden ratio (phi grid), diagonals at 45 degrees from each
// corner, the golden triangle and the golden spiral. Triangle and spiral can
// be flipped to any corner.
import type { Box } from "./picture";

export type Guide = "thirds" | "golden" | "diagonal" | "triangle" | "spiral";

export const GUIDES: { value: Guide; label: string }[] = [
  { value: "thirds", label: "Thirds" },
  { value: "golden", label: "Golden" },
  { value: "diagonal", label: "Diagonal" },
  { value: "triangle", label: "Triangle" },
  { value: "spiral", label: "Spiral" },
];

const PHI = (1 + Math.sqrt(5)) / 2;

type Props = {
  guide: Guide;
  /** 0..3: which corner the triangle or spiral turns to (mirrored across, then down). */
  flip: number;
  box: Box | null;
};

export function Composition({ guide, flip, box }: Props) {
  if (!box) return null;
  const { width: w, height: h } = box;
  // Mirror across (flip 1, 3) and down (flip 2, 3).
  const mirror = `translate(${flip & 1 ? w : 0} ${flip & 2 ? h : 0}) scale(${flip & 1 ? -1 : 1} ${flip & 2 ? -1 : 1})`;
  return (
    <svg className="composition" style={{ left: box.left, top: box.top, width: w, height: h }} viewBox={`0 0 ${w} ${h}`}>
      <g transform={guide === "triangle" || guide === "spiral" ? mirror : undefined}>{lines(guide, w, h)}</g>
    </svg>
  );
}

function lines(guide: Guide, w: number, h: number) {
  switch (guide) {
    case "thirds":
      return grid(w, h, 1 / 3);
    case "golden":
      return grid(w, h, 1 / PHI ** 2);
    case "diagonal":
      return [
        <line key="1" x1={0} y1={0} x2={h} y2={h} />,
        <line key="2" x1={w} y1={0} x2={w - h} y2={h} />,
        <line key="3" x1={0} y1={h} x2={h} y2={0} />,
        <line key="4" x1={w} y1={h} x2={w - h} y2={0} />,
      ];
    case "triangle": {
      // The diagonal from bottom-left to top-right, and from the other two
      // corners the lines meeting it at right angles.
      const foot = (px: number, py: number) => {
        const t = (px * w + (py - h) * -h) / (w * w + h * h);
        return [t * w, h - t * h];
      };
      const [ax, ay] = foot(0, 0);
      const [bx, by] = foot(w, h);
      return [
        <line key="d" x1={0} y1={h} x2={w} y2={0} />,
        <line key="a" x1={0} y1={0} x2={ax} y2={ay} />,
        <line key="b" x1={w} y1={h} x2={bx} y2={by} />,
      ];
    }
    case "spiral":
      return <path d={spiral()} transform={`scale(${w / PHI} ${h})`} />;
  }
}

/** Lines at `f` and `1 - f` of the width and of the height. */
function grid(w: number, h: number, f: number) {
  return [f, 1 - f].flatMap((at) => [
    <line key={`x${at}`} x1={w * at} y1={0} x2={w * at} y2={h} />,
    <line key={`y${at}`} x1={0} y1={h * at} x2={w} y2={h * at} />,
  ]);
}

/**
 * The golden spiral in a PHI x 1 rectangle: cut a square off it (left, top,
 * right, bottom, in turn), draw a quarter circle in that square, and go on in
 * what is left.
 */
function spiral(): string {
  let [x, y, w, h] = [0, 0, PHI, 1];
  let d = `M 0 1`;
  for (let i = 0; i < 12; i++) {
    const s = Math.min(w, h);
    const arc = (ex: number, ey: number) => (d += ` A ${s} ${s} 0 0 1 ${ex} ${ey}`);
    switch (i % 4) {
      case 0: // square on the left: bottom-left to top-right
        arc(x + s, y);
        [x, w] = [x + s, w - s];
        break;
      case 1: // on top: top-left to bottom-right
        arc(x + s, y + s);
        [y, h] = [y + s, h - s];
        break;
      case 2: // on the right: top-right to bottom-left
        arc(x + w - s, y + s);
        w -= s;
        break;
      case 3: // at the bottom: bottom-right to top-left
        arc(x, y + h - s);
        h -= s;
        break;
    }
  }
  return d;
}

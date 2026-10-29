// Composition guides over the picture, matching Argentum's crop overlays:
// thirds, diagonals, golden triangle, golden spiral, phi grid, armature and
// centre lines. Triangle and spiral turn in quarter turns (Rotate): the spiral
// runs along the frame's width, then along its height, for portrait framing.
import type { Box } from "./picture";

export type Guide = "thirds" | "diagonal" | "triangle" | "spiral" | "phi" | "armature" | "center";

export const GUIDES: { value: Guide; label: string }[] = [
  { value: "thirds", label: "Thirds" },
  { value: "diagonal", label: "Diagonal" },
  { value: "triangle", label: "Triangle" },
  { value: "spiral", label: "Spiral" },
  { value: "phi", label: "Phi grid" },
  { value: "armature", label: "Armature" },
  { value: "center", label: "Center" },
];

/** The guides Rotate turns. */
export const TURNING: Guide[] = ["triangle", "spiral"];

const PHI = (1 + Math.sqrt(5)) / 2;

type Props = {
  guide: Guide;
  /** Quarter turns, 0..3. */
  rotation: number;
  box: Box | null;
};

export function Composition({ guide, rotation, box }: Props) {
  if (!box) return null;
  const { width: w, height: h } = box;
  return (
    <svg className="composition" style={{ left: box.left, top: box.top, width: w, height: h }} viewBox={`0 0 ${w} ${h}`}>
      {lines(guide, w, h, rotation % 4)}
    </svg>
  );
}

/** A small drawing of `guide` for its tile, from the same lines as the overlay. */
export function GuideArt({ guide, rotation }: { guide: Guide; rotation: number }) {
  return (
    <svg className="art" viewBox="-1 -1 32 22" aria-hidden="true">
      <rect width={30} height={20} rx={1.5} />
      {lines(guide, 30, 20, rotation % 4)}
    </svg>
  );
}

function lines(guide: Guide, w: number, h: number, r: number) {
  switch (guide) {
    case "thirds":
      return grid(w, h, 1 / 3);
    case "phi":
      return grid(w, h, 1 / PHI ** 2);
    case "center":
      return [<line key="x" x1={w / 2} y1={0} x2={w / 2} y2={h} />, <line key="y" x1={0} y1={h / 2} x2={w} y2={h / 2} />];
    case "diagonal":
      // From each corner to the point two thirds along the opposite side.
      return [
        <line key="1" x1={0} y1={0} x2={(w * 2) / 3} y2={h} />,
        <line key="2" x1={w / 3} y1={0} x2={w} y2={h} />,
        <line key="3" x1={w} y1={0} x2={w / 3} y2={h} />,
        <line key="4" x1={(w * 2) / 3} y1={0} x2={0} y2={h} />,
      ];
    case "triangle":
      return triangle(w, h, r % 2 === 1);
    case "armature":
      return armature(w, h);
    case "spiral": {
      // Drawn in a PHI x 1 box, turned, then stretched over the frame: along
      // its width at 0 and 2 quarter turns, along its height at 1 and 3.
      const [along, across] = r % 2 === 0 ? [w, h] : [h, w];
      const transform = `translate(${w / 2} ${h / 2}) rotate(${r * 90}) scale(${along / PHI} ${across}) translate(${-PHI / 2} -0.5)`;
      return <path d={spiral()} transform={transform} />;
    }
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
 * A diagonal, and from the other two corners the lines meeting it at right
 * angles: bottom-left to top-right, or (`other`) top-left to bottom-right.
 */
function triangle(w: number, h: number, other: boolean) {
  const [ax, ay, bx, by] = other ? [0, 0, w, h] : [0, h, w, 0];
  const corners = other
    ? [
        [w, 0],
        [0, h],
      ]
    : [
        [0, 0],
        [w, h],
      ];
  const foot = ([px, py]: number[]) => {
    const [dx, dy] = [bx - ax, by - ay];
    const t = ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy);
    return [ax + t * dx, ay + t * dy];
  };
  return [
    <line key="d" x1={ax} y1={ay} x2={bx} y2={by} />,
    ...corners.map((corner, i) => {
      const [fx, fy] = foot(corner);
      return <line key={i} x1={corner[0]} y1={corner[1]} x2={fx} y2={fy} />;
    }),
  ];
}

/**
 * The armature of the rectangle: both diagonals, from each corner the line at
 * right angles to the diagonal it doesn't sit on, and (dashed) the verticals
 * and horizontals through where those lines cross the diagonals.
 */
function armature(w: number, h: number) {
  const m = h / w;
  const reach = h / (m * m); // where the right-angle lines meet the far side (beyond the frame for wide frames)
  const x = (m * m * w) / (m * m + 1);
  const y = m * x;
  return [
    <line key="d1" x1={0} y1={0} x2={w} y2={h} />,
    <line key="d2" x1={w} y1={0} x2={0} y2={h} />,
    <line key="r1" x1={w} y1={0} x2={0} y2={reach} />,
    <line key="r2" x1={0} y1={0} x2={w} y2={reach} />,
    <line key="r3" x1={0} y1={h} x2={w} y2={h - reach} />,
    <line key="r4" x1={w} y1={h} x2={0} y2={h - reach} />,
    ...[
      [w - x, 0, w - x, h],
      [x, 0, x, h],
      [0, y, w, y],
      [0, h - y, w, h - y],
    ].map(([x1, y1, x2, y2], i) => <line key={`m${i}`} className="dashed" x1={x1} y1={y1} x2={x2} y2={y2} />),
  ];
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

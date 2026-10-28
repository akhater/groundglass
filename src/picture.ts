// Where the live picture sits: the canvas fits it inside its box
// (object-fit: contain), leaving bars on two sides.

export type Box = { left: number; top: number; width: number; height: number };

/** The picture's area in pixels, relative to the canvas's parent; null before the first frame. */
export function pictureBox(canvas: HTMLCanvasElement): Box | null {
  const parent = canvas.parentElement;
  if (!canvas.width || !parent) return null;
  const box = canvas.getBoundingClientRect();
  const origin = parent.getBoundingClientRect();
  const scale = Math.min(box.width / canvas.width, box.height / canvas.height);
  const width = canvas.width * scale;
  const height = canvas.height * scale;
  return {
    left: box.left - origin.left + (box.width - width) / 2,
    top: box.top - origin.top + (box.height - height) / 2,
    width,
    height,
  };
}

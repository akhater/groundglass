// Where the live picture sits: the canvas fits it inside its box
// (object-fit: contain), leaving bars on two sides.

export type Box = { left: number; top: number; width: number; height: number };

/**
 * The picture's area in pixels, relative to the canvas's parent; null before the
 * first frame. Turned `turns` quarter turns, upright for portrait shooting: taken
 * from the turns, not the canvas, which takes the new shape only at the next frame.
 */
export function pictureBox(canvas: HTMLCanvasElement, turns: number): Box | null {
  const parent = canvas.parentElement;
  if (!canvas.width || !parent) return null;
  const box = canvas.getBoundingClientRect();
  const origin = parent.getBoundingClientRect();
  // Live view is wider than tall, the sensor's way up.
  const [long, short] = [Math.max(canvas.width, canvas.height), Math.min(canvas.width, canvas.height)];
  const [frameWidth, frameHeight] = turns % 2 ? [short, long] : [long, short];
  const scale = Math.min(box.width / frameWidth, box.height / frameHeight);
  const width = frameWidth * scale;
  const height = frameHeight * scale;
  return {
    left: box.left - origin.left + (box.width - width) / 2,
    top: box.top - origin.top + (box.height - height) / 2,
    width,
    height,
  };
}

// The camera's AF points over the picture, as on the camera: green once
// One-Shot holds focus, blue while Servo tracks. Servo's whole AF area isn't drawn.
import type * as camera from "./camera";
import type { Box } from "./picture";

export function AfFrames({ frames, box }: { frames: camera.AfFrame[]; box: Box | null }) {
  if (!box) return null;
  return frames
    .filter((frame) => frame.state !== "area")
    .map((frame, i) => (
      <div
        key={i}
        className={`af-frame ${frame.state}`}
        style={{
          left: box.left + frame.x * box.width,
          top: box.top + frame.y * box.height,
          width: frame.width * box.width,
          height: frame.height * box.height,
        }}
      />
    ));
}

// The camera's AF frames over the picture, coloured once it holds focus:
// green in One-Shot, blue in Servo, as on the camera.
import type * as camera from "./camera";
import type { Box } from "./picture";

type Props = { frames: camera.AfFrame[]; box: Box | null; servo: boolean };

export function AfFrames({ frames, box, servo }: Props) {
  if (!box) return null;
  return frames.map((frame, i) => (
    <div
      key={i}
      className={`af-frame ${frame.focused ? (servo ? "servo" : "focused") : ""}`}
      style={{
        left: box.left + frame.x * box.width,
        top: box.top + frame.y * box.height,
        width: frame.width * box.width,
        height: frame.height * box.height,
      }}
    />
  ));
}

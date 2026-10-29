// Tap (or click) the picture to move the camera's AF point there, as on the
// camera's touchscreen. Only moves the point: focusing stays with AF-ON. The
// camera's own AF frame is drawn from what it reports (AfFrames).
import { useState, type MouseEvent, type RefObject } from "react";
import * as camera from "./camera";
import { pictureBox } from "./picture";

export function useTapFocus(canvas: RefObject<HTMLCanvasElement | null>) {
  const [error, setError] = useState("");

  /**
   * Moves the AF point to the tapped spot on the picture as shown, `turns` quarter
   * turns from the sensor's way up; taps on the bars around the picture do nothing.
   */
  async function focusAt(e: MouseEvent<HTMLElement>, turns: number) {
    const view = canvas.current;
    const box = view && pictureBox(view, turns);
    const origin = view?.parentElement?.getBoundingClientRect();
    if (!box || !origin) return;
    const x = (e.clientX - origin.left - box.left) / box.width;
    const y = (e.clientY - origin.top - box.top) / box.height;
    if (x < 0 || x > 1 || y < 0 || y > 1) return;
    try {
      await camera.setFocusPoint(x, y, turns);
      setError("");
    } catch (err) {
      setError(camera.describe(err));
    }
  }

  return { error, focusAt };
}

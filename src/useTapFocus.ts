// Tap (or click) the picture to move the camera's AF point there, as on the
// camera's touchscreen. Only moves the point: focusing stays with AF-ON.
import { useEffect, useState, type MouseEvent, type RefObject } from "react";
import * as camera from "./camera";

/** Where the last tap landed, in pixels within the element that was tapped. */
type Spot = { x: number; y: number };

/** The marker goes when live view stops: the next start may have moved the point. */
export function useTapFocus(canvas: RefObject<HTMLCanvasElement | null>, live: boolean) {
  const [spot, setSpot] = useState<Spot | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!live) setSpot(null);
  }, [live]);

  /** Moves the AF point to the tapped spot; taps on the bars around the picture do nothing. */
  async function focusAt(e: MouseEvent<HTMLElement>) {
    const view = canvas.current;
    if (!view || !view.width) return;
    // The picture is fitted inside the canvas box (object-fit: contain).
    const box = view.getBoundingClientRect();
    const scale = Math.min(box.width / view.width, box.height / view.height);
    const width = view.width * scale;
    const height = view.height * scale;
    const x = (e.clientX - box.left - (box.width - width) / 2) / width;
    const y = (e.clientY - box.top - (box.height - height) / 2) / height;
    if (x < 0 || x > 1 || y < 0 || y > 1) return;
    const stage = e.currentTarget.getBoundingClientRect();
    setSpot({ x: e.clientX - stage.left, y: e.clientY - stage.top });
    try {
      await camera.setFocusPoint(x, y);
      setError("");
    } catch (err) {
      setError(camera.describe(err));
    }
  }

  return { spot, error, focusAt };
}

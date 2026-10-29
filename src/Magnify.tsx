// Magnification (one button through x1, x5 and x10) and depth of field
// preview, usable only where the camera has them (EOS Utility mode). Both go back to off whenever live view
// stops or restarts, as the camera does.
import { useEffect, useState, type RefObject } from "react";
import * as camera from "./camera";

const NEXT: Record<camera.Zoom, camera.Zoom> = { 1: 5, 5: 10, 10: 1 };

/** `zoom` is read by the frame loop, which shows x10 as the central half of x5. */
export function useMagnify(live: boolean, zoom: RefObject<camera.Zoom>) {
  const [level, setLevel] = useState<camera.Zoom>(1);
  const [preview, setPreview] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (live) return;
    zoom.current = 1;
    setLevel(1);
    setPreview(false);
  }, [live, zoom]);

  /** x1, x5, x10, then back to x1, as the camera's magnify button does. */
  async function magnify() {
    const next = NEXT[level];
    try {
      await camera.zoom(next);
      zoom.current = next;
      setLevel(next);
      setError("");
    } catch (e) {
      setError(camera.describe(e));
    }
  }

  async function togglePreview() {
    try {
      await camera.depthOfFieldPreview(!preview);
      setPreview(!preview);
      setError("");
    } catch (e) {
      setError(camera.describe(e));
    }
  }

  return { level, preview, error, magnify, togglePreview };
}

type Props = { camera: camera.CameraInfo; magnify: ReturnType<typeof useMagnify> };

/** Dimmed where the camera can't (CCAPI mode), so they are there to find. */
export function MagnifyButtons({ camera: info, magnify }: Props) {
  return (
    <>
      <button
        className="loupe"
        disabled={!info.zoom}
        aria-label={`Magnify, now ×${magnify.level}`}
        aria-pressed={magnify.level > 1}
        onClick={() => void magnify.magnify()}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <circle cx="6.5" cy="6.5" r="4.5" />
          <path d="M10 10l4.5 4.5" />
        </svg>
        ×{magnify.level}
      </button>
      <button
        disabled={!info.depthOfFieldPreview}
        aria-pressed={magnify.preview}
        onClick={() => void magnify.togglePreview()}
      >
        DOF
      </button>
    </>
  );
}

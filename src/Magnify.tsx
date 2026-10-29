// x5 / x10 magnification and depth of field preview, shown only where the
// camera has them (EOS Utility mode). Both go back to off whenever live view
// stops or restarts, as the camera does.
import { useEffect, useState, type RefObject } from "react";
import * as camera from "./camera";

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

  /** Tapping the level already on goes back to the whole picture. */
  async function magnify(wanted: camera.Zoom) {
    const next = level === wanted ? 1 : wanted;
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

export function MagnifyButtons({ camera: info, magnify }: Props) {
  return (
    <>
      {info.zoom &&
        ([5, 10] as const).map((level) => (
          <button key={level} aria-pressed={magnify.level === level} onClick={() => void magnify.magnify(level)}>
            ×{level}
          </button>
        ))}
      {info.depthOfFieldPreview && (
        <button aria-pressed={magnify.preview} onClick={() => void magnify.togglePreview()}>
          DOF
        </button>
      )}
    </>
  );
}

// Magnification (one button through x1, x5 and x10) and depth of field
// preview, usable only where the camera has them (EOS Utility mode). Both go back to off whenever live view
// stops or restarts, as the camera does.
import { useEffect, useRef, useState, type RefObject } from "react";
import * as camera from "./camera";
import { Tool } from "./Controls";

const NEXT: Record<camera.Zoom, camera.Zoom> = { 1: 5, 5: 10, 10: 1 };

/** `zoom` is read by the frame loop, which shows x10 as the central half of x5. */
export function useMagnify(live: boolean, zoom: RefObject<camera.Zoom>) {
  const [level, setLevel] = useState<camera.Zoom>(1);
  /** The level last asked of the camera, ahead of `level` while a request is out, so quick taps step on. */
  const asked = useRef<camera.Zoom>(1);
  const [preview, setPreview] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (live) return;
    zoom.current = 1;
    asked.current = 1;
    setLevel(1);
    setPreview(false);
  }, [live, zoom]);

  /** x1, x5, x10, then back to x1, as the camera's magnify button does. */
  async function magnify() {
    const before = asked.current;
    const next = NEXT[before];
    asked.current = next;
    try {
      await camera.zoom(next);
      zoom.current = next;
      setLevel(next);
      setError("");
    } catch (e) {
      // The camera stayed where it was, unless a later tap has asked again since.
      if (asked.current === next) asked.current = before;
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

type Props = { camera: camera.CameraInfo | null; magnify: ReturnType<typeof useMagnify> };

/** Dimmed where the camera can't (CCAPI mode, or not live), so they are there to find. */
export function MagnifyTools({ camera: info, magnify }: Props) {
  return (
    <>
      <Tool
        icon="magnify"
        label="Magnify"
        value={`×${magnify.level}`}
        on={magnify.level > 1}
        disabled={!info?.zoom}
        onClick={() => void magnify.magnify()}
      />
      <Tool
        icon="dof"
        label="Depth of field preview"
        value={magnify.preview ? "On" : "Off"}
        on={magnify.preview}
        disabled={!info?.depthOfFieldPreview}
        onClick={() => void magnify.togglePreview()}
      />
    </>
  );
}

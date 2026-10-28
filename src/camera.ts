// The Rust commands in src-tauri/src/lib.rs, typed.
import { invoke } from "@tauri-apps/api/core";

export type CameraInfo = { model: string; firmware: string };

/** Mirrors `ccapi::CameraError`. */
export type CameraError =
  | { kind: "Unreachable"; detail: string }
  | { kind: "LiveViewOff" }
  | { kind: "Busy" }
  | { kind: "Refused"; detail: string };

export const connect = (address: string) => invoke<CameraInfo>("connect", { address });

export const startLiveView = (cameraDisplayOn: boolean) =>
  invoke<void>("start_live_view", { cameraDisplayOn });

export const stopLiveView = () => invoke<void>("stop_live_view");

/** The newest live view frame as JPEG bytes. */
export const frame = () => invoke<ArrayBuffer>("frame");

/** Looks: .cube files kept in the app's data folder. */
export const listLuts = () => invoke<string[]>("list_luts");
export const saveLut = (name: string, text: string) => invoke<void>("save_lut", { name, text });
export const readLut = (name: string) => invoke<string>("read_lut", { name });

/** Errors that restarting live view fixes, with no need to reconnect. */
export function isRestartable(error: unknown): boolean {
  const kind = (error as CameraError)?.kind;
  return kind === "LiveViewOff" || kind === "Busy";
}

export function describe(error: unknown): string {
  const e = error as CameraError;
  switch (e?.kind) {
    case "Unreachable":
      return "Can't reach the camera. Is it on, and on the same Wi-Fi?";
    case "LiveViewOff":
      return "The camera stopped live view.";
    case "Busy":
      return "The camera is busy.";
    case "Refused":
      return `The camera refused: ${e.detail}`;
    default:
      return String(error);
  }
}

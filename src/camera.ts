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

/** Takes a photo with autofocus, like a full press of the shutter button. */
export const shoot = () => invoke<void>("shoot");

/** Back-button focus: true on press, false on release. */
export const autofocus = (held: boolean) => invoke<void>("autofocus", { held });

/** Mirrors `ccapi::ExposureSetting`. */
export type ExposureSetting = "iso" | "av" | "tv";
/** `choices` is empty when the shooting mode sets the value (e.g. aperture in Tv). */
export type SettingState = { value: string; choices: string[] };

export const exposureSetting = (setting: ExposureSetting) =>
  invoke<SettingState>("exposure_setting", { setting });
export const setExposureSetting = (setting: ExposureSetting, value: string) =>
  invoke<void>("set_exposure_setting", { setting, value });

/** Mirrors `ccapi::Level`: degrees from level. */
export type Level = { roll: number; pitch: number };
/** The camera's level while live, null if the camera gives none. */
export const level = () => invoke<Level | null>("level");

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

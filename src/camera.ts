// The Rust commands in src-tauri/src/lib.rs, typed.
import { Channel, invoke } from "@tauri-apps/api/core";

/** Mirrors `camera::CameraInfo`: `zoom` and `depthOfFieldPreview` say what live view can do (EOS Utility mode only). */
export type CameraInfo = { model: string; firmware: string; zoom: boolean; depthOfFieldPreview: boolean };

/** Mirrors `camera::CameraError`. */
export type CameraError =
  | { kind: "Unreachable"; detail: string }
  | { kind: "LiveViewOff" }
  | { kind: "Busy" }
  | { kind: "Refused"; detail: string }
  | { kind: "PairingRefused" };

/** `onPairing` runs when the camera is in EOS Utility mode, where the first connect waits for OK on the camera. */
export function connect(address: string, onPairing: () => void) {
  const pairing = new Channel<boolean>();
  pairing.onmessage = onPairing;
  return invoke<CameraInfo>("connect", { address, pairing });
}

export const startLiveView = (cameraDisplayOn: boolean) =>
  invoke<void>("start_live_view", { cameraDisplayOn });

export const stopLiveView = () => invoke<void>("stop_live_view");

/** The newest live view frame as JPEG bytes. */
export const frame = () => invoke<ArrayBuffer>("frame");

/** Takes a photo without autofocus (focus is AF-ON's job), like a full press of the shutter button. */
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

/** Moves the AF point to (x, y), fractions across the picture. */
export const setFocusPoint = (x: number, y: number) => invoke<void>("set_focus_point", { x, y });

/** Live view's magnification: the whole picture, x5 or x10. */
export type Zoom = 1 | 5 | 10;
/** Where `CameraInfo.zoom` says the camera can. At x10 frames still come at x5: liveView.ts shows their central half. */
export const zoom = (level: Zoom) => invoke<void>("zoom", { level });
/** Where `CameraInfo.depthOfFieldPreview` says the camera can: the lens stops down while on. */
export const depthOfFieldPreview = (on: boolean) => invoke<void>("depth_of_field_preview", { on });

/** Mirrors `camera::Level`: degrees from level. */
export type Level = { roll: number; pitch: number };
/** Mirrors `camera::AfState`. */
export type AfState = "waiting" | "focused" | "tracking" | "area";
/** Mirrors `camera::AfFrame`: fractions of the picture. */
export type AfFrame = { x: number; y: number; width: number; height: number; state: AfState };
/** Mirrors `camera::LiveDetails`. */
export type LiveDetails = { level: Level | null; afFrames: AfFrame[] };
/** The level and AF frames, while live. */
export const liveDetails = () => invoke<LiveDetails>("live_details");

/** Looks: .cube files kept in the app's data folder. */
export const listLuts = () => invoke<string[]>("list_luts");
export const saveLut = (name: string, text: string) => invoke<void>("save_lut", { name, text });
export const readLut = (name: string) => invoke<string>("read_lut", { name });

/** The camera can't answer this instant (e.g. a photo is being taken). */
export const isBusy = (error: unknown) => (error as CameraError)?.kind === "Busy";

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
    case "PairingRefused":
      return "The camera refused to pair: choose Groundglass again in its Wi-Fi menu.";
    default:
      return String(error);
  }
}

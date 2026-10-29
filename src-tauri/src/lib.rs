pub mod camera;
pub mod canon_codes;
pub mod ccapi;
mod ettr;
mod ettr_run;
mod eos;
mod luts;
mod orientation;
pub mod ptpip;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use camera::{Battery, Camera, CameraError, CameraInfo, SettingState};
use ccapi::ExposureSetting;
use ettr_run::Outcome;
use orientation::Upright;
use tauri::{AppHandle, Manager, RunEvent, State, ipc::Channel, ipc::Response};

/// The connected camera, if any. Calls clone the `Arc` and talk to the camera
/// without holding the lock, so a slow frame never blocks a stop.
#[derive(Default)]
struct Connection(Mutex<Option<Arc<Camera>>>);

impl Connection {
    fn camera(&self) -> Result<Arc<Camera>, CameraError> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .ok_or_else(|| CameraError::Unreachable("Not connected".into()))
    }
}

#[tauri::command(async)]
/// `pairing` hears once when the camera is in EOS Utility mode, so the screen can say to press OK.
fn connect(address: String, pairing: Channel<bool>, connection: State<Connection>) -> Result<CameraInfo, CameraError> {
    // Let go of the last camera first: over PTP the camera takes one session at a time.
    drop(connection.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take());
    let (camera, info) = Camera::connect(&address, || {
        let _ = pairing.send(true);
    })?;
    *connection.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Arc::new(camera));
    Ok(info)
}

#[tauri::command(async)]
fn start_live_view(camera_display_on: bool, connection: State<Connection>) -> Result<(), CameraError> {
    connection.camera()?.start_live_view(camera_display_on)
}

#[tauri::command(async)]
fn stop_live_view(connection: State<Connection>) -> Result<(), CameraError> {
    connection.camera()?.stop_live_view()
}

/// JPEG bytes, delivered to the page as an ArrayBuffer.
#[tauri::command(async)]
fn frame(connection: State<Connection>) -> Result<Response, CameraError> {
    connection.camera()?.frame().map(Response::new)
}

#[tauri::command(async)]
fn shoot(connection: State<Connection>) -> Result<(), CameraError> {
    connection.camera()?.shoot()
}

/// Back-button focus: `true` on press, `false` on release.
#[tauri::command(async)]
fn autofocus(held: bool, connection: State<Connection>) -> Result<(), CameraError> {
    connection.camera()?.autofocus(held)
}

#[tauri::command(async)]
fn exposure_setting(setting: ExposureSetting, connection: State<Connection>) -> Result<SettingState, CameraError> {
    connection.camera()?.exposure_setting(setting)
}

#[tauri::command(async)]
fn set_exposure_setting(
    setting: ExposureSetting,
    value: String,
    connection: State<Connection>,
) -> Result<(), CameraError> {
    connection.camera()?.set_exposure_setting(setting, &value)
}

/// Expose to the right: moves the settings not `pinned` to the brightest exposure
/// that clips nothing, in a few rounds of set, wait, read (ettr_run.rs).
#[tauri::command(async)]
fn expose_to_the_right(pinned: Vec<ExposureSetting>, connection: State<Connection>) -> Result<Outcome, CameraError> {
    ettr_run::run(connection.camera()?.as_ref(), &pinned)
}

/// Turned upright from the camera's roll: `turns` is what the screen shows now,
/// kept as it is while `locked` (rotation lock).
#[tauri::command(async)]
fn live_details(turns: u8, locked: bool, connection: State<Connection>) -> Result<Upright, CameraError> {
    Ok(orientation::upright(connection.camera()?.live_details()?, turns, locked))
}

/// (`x`, `y`) on the picture as shown, turned `turns` quarter turns clockwise.
#[tauri::command(async)]
fn set_focus_point(x: f64, y: f64, turns: u8, connection: State<Connection>) -> Result<(), CameraError> {
    let (x, y) = orientation::sensor_point(x, y, turns);
    connection.camera()?.set_focus_point(x, y)
}

/// Live view magnified 1, 5 or 10 times.
#[tauri::command(async)]
fn zoom(level: u32, connection: State<Connection>) -> Result<(), CameraError> {
    connection.camera()?.zoom(level)
}

#[tauri::command(async)]
fn depth_of_field_preview(on: bool, connection: State<Connection>) -> Result<(), CameraError> {
    connection.camera()?.depth_of_field_preview(on)
}

/// The battery's charge, or null when the camera doesn't say.
#[tauri::command(async)]
fn battery(connection: State<Connection>) -> Result<Option<Battery>, CameraError> {
    connection.camera()?.battery()
}

fn luts_folder(app: &AppHandle) -> Result<PathBuf, String> {
    let data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    luts::folder(&data).map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn list_luts(app: AppHandle) -> Result<Vec<String>, String> {
    luts::list(&luts_folder(&app)?).map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn save_lut(name: String, text: String, app: AppHandle) -> Result<(), String> {
    luts::save(&luts_folder(&app)?, &name, &text).map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn read_lut(name: String, app: AppHandle) -> Result<String, String> {
    luts::read(&luts_folder(&app)?, &name).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Connection::default())
        .invoke_handler(tauri::generate_handler![
            connect,
            start_live_view,
            stop_live_view,
            frame,
            shoot,
            autofocus,
            exposure_setting,
            set_exposure_setting,
            expose_to_the_right,
            live_details,
            set_focus_point,
            zoom,
            depth_of_field_preview,
            battery,
            list_luts,
            save_lut,
            read_lut
        ])
        .build(tauri::generate_context!())
        .expect("error while building Groundglass")
        .run(|app, event| {
            // Leave the camera as we found it: live view off, its screen on, its Picture Style back,
            // and over PTP the session closed. `is_live` says when that is needed and safe.
            if let RunEvent::Exit = event
                && let Ok(camera) = app.state::<Connection>().camera()
                && camera.is_live()
            {
                let _ = camera.stop_live_view();
            }
        });
}

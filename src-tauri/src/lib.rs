pub mod ccapi;
mod luts;
pub mod ptpip;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ccapi::{Camera, CameraError, CameraInfo, ExposureSetting, LiveDetails, SettingState};
use tauri::{AppHandle, Manager, RunEvent, State, ipc::Response};

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
fn connect(address: String, connection: State<Connection>) -> Result<CameraInfo, CameraError> {
    let (camera, info) = Camera::connect(&address)?;
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
    // No autofocus from the shutter: focus stays on AF-ON (back-button focus),
    // as a camera with focus split from the shutter expects.
    connection.camera()?.shoot(false)
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

#[tauri::command(async)]
fn live_details(connection: State<Connection>) -> Result<LiveDetails, CameraError> {
    connection.camera()?.live_details()
}

#[tauri::command(async)]
fn set_focus_point(x: f64, y: f64, connection: State<Connection>) -> Result<(), CameraError> {
    connection.camera()?.set_focus_point(x, y)
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
            live_details,
            set_focus_point,
            list_luts,
            save_lut,
            read_lut
        ])
        .build(tauri::generate_context!())
        .expect("error while building Groundglass")
        .run(|app, event| {
            // Leave the camera as we found it: live view off, its screen on, its Picture Style back.
            // Only while live: after a long idle the camera may have closed the
            // session, and a session must not be reopened by anything but `connect`.
            if let RunEvent::Exit = event
                && let Ok(camera) = app.state::<Connection>().camera()
                && camera.is_live()
            {
                let _ = camera.stop_live_view();
            }
        });
}

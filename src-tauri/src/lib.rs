pub mod ccapi;

use std::sync::{Arc, Mutex};

use ccapi::{Camera, CameraError, CameraInfo};
use tauri::{Manager, RunEvent, State, ipc::Response};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Connection::default())
        .invoke_handler(tauri::generate_handler![connect, start_live_view, stop_live_view, frame])
        .build(tauri::generate_context!())
        .expect("error while building Groundglass")
        .run(|app, event| {
            // Leave the camera as we found it: live view off, its screen on.
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

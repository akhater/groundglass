//! One tap from the phone to the camera's Wi-Fi: the Android code (`android/`)
//! wakes the paired camera over Bluetooth, starts its Wi-Fi and joins it, with
//! the app's traffic bound to that network. Elsewhere it reports that it can't.

use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

/// How far a connect got. `failure` says what to do, in one sentence.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub woke: bool,
    pub wifi_started: bool,
    pub joined: bool,
    pub address: Option<String>,
    pub failure: Option<String>,
}

/// Kotlin takes an object even for a command with no arguments.
#[cfg(target_os = "android")]
#[derive(Serialize)]
struct NoArguments {}

#[cfg(target_os = "android")]
pub struct CameraLink<R: Runtime>(tauri::plugin::PluginHandle<R>);

#[cfg(target_os = "android")]
impl<R: Runtime> CameraLink<R> {
    /// Blocks until the phone has joined the camera's Wi-Fi or given up.
    pub fn connect(&self) -> Result<Link, String> {
        self.0.run_mobile_plugin("connect", NoArguments {}).map_err(|e| e.to_string())
    }

    /// Lets go of the camera's Wi-Fi and Bluetooth; the phone's traffic goes back to normal.
    pub fn release(&self) -> Result<(), String> {
        self.0.run_mobile_plugin("release", NoArguments {}).map_err(|e| e.to_string())
    }
}

#[cfg(not(target_os = "android"))]
pub struct CameraLink<R: Runtime>(std::marker::PhantomData<fn() -> R>);

#[cfg(not(target_os = "android"))]
impl<R: Runtime> CameraLink<R> {
    pub fn connect(&self) -> Result<Link, String> {
        Err("Bluetooth connect is on Android only.".into())
    }

    pub fn release(&self) -> Result<(), String> {
        Ok(())
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("camera-link")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            let link = CameraLink(api.register_android_plugin("co.groundglass.cameralink", "CameraLinkPlugin")?);
            #[cfg(not(target_os = "android"))]
            let link = {
                let _ = api;
                CameraLink::<R>(std::marker::PhantomData)
            };
            app.manage(link);
            Ok(())
        })
        .build()
}

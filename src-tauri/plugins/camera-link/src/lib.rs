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
    /// How the camera took the app's id and name, write by write, when it refused one.
    pub registration: Option<String>,
    /// What the camera last said at the step that stopped.
    pub detail: Option<String>,
    pub failure: Option<String>,
}

/// Kotlin takes an object even for a command with no arguments.
#[cfg(target_os = "android")]
#[derive(Serialize)]
struct NoArguments {}

#[cfg(target_os = "android")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectArguments<'a> {
    client_id: &'a [u8],
    client_name: &'a str,
}

#[cfg(target_os = "android")]
#[derive(Serialize)]
struct KeepAwakeArguments {
    on: bool,
}

#[cfg(target_os = "android")]
#[derive(Deserialize)]
struct Readiness {
    ready: bool,
}

#[cfg(target_os = "android")]
#[derive(Deserialize)]
struct Allowed {
    allowed: bool,
}

#[cfg(target_os = "android")]
pub struct CameraLink<R: Runtime>(tauri::plugin::PluginHandle<R>);

#[cfg(target_os = "android")]
impl<R: Runtime> CameraLink<R> {
    /// Whether a connect can start now: Bluetooth and Wi-Fi are on.
    pub fn ready(&self) -> Result<bool, String> {
        let answer = self.0.run_mobile_plugin::<Readiness>("ready", NoArguments {});
        answer.map(|readiness| readiness.ready).map_err(|e| e.to_string())
    }

    /// Whether the app may reach the local network, asking first where Android
    /// 17 requires it. Without it, any connect to a camera times out.
    pub fn local_network(&self) -> Result<bool, String> {
        let answer = self.0.run_mobile_plugin::<Allowed>("localNetwork", NoArguments {});
        answer.map(|allowed| allowed.allowed).map_err(|e| e.to_string())
    }

    /// Blocks until the phone has joined the camera's Wi-Fi or given up. At a
    /// first pairing the camera learns `client_id` and `client_name`, who its
    /// PTP/IP Init will come from.
    pub fn connect(&self, client_id: &[u8; 16], client_name: &str) -> Result<Link, String> {
        let arguments = ConnectArguments { client_id, client_name };
        self.0.run_mobile_plugin("connect", arguments).map_err(|e| e.to_string())
    }

    /// Lets go of the camera's Wi-Fi and Bluetooth; the phone's traffic goes back to normal.
    pub fn release(&self) -> Result<(), String> {
        self.0.run_mobile_plugin("release", NoArguments {}).map_err(|e| e.to_string())
    }

    /// Keeps the phone's screen on while `on`.
    pub fn keep_awake(&self, on: bool) -> Result<(), String> {
        self.0.run_mobile_plugin("keepAwake", KeepAwakeArguments { on }).map_err(|e| e.to_string())
    }
}

#[cfg(not(target_os = "android"))]
pub struct CameraLink<R: Runtime>(std::marker::PhantomData<fn() -> R>);

#[cfg(not(target_os = "android"))]
impl<R: Runtime> CameraLink<R> {
    pub fn ready(&self) -> Result<bool, String> {
        Ok(false)
    }

    pub fn local_network(&self) -> Result<bool, String> {
        Ok(true)
    }

    pub fn connect(&self, _client_id: &[u8; 16], _client_name: &str) -> Result<Link, String> {
        Err("Bluetooth connect is on Android only.".into())
    }

    pub fn release(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn keep_awake(&self, _on: bool) -> Result<(), String> {
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

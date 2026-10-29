//! The camera as the app sees it, over either of its Wi-Fi functions: CCAPI
//! (ccapi.rs) or "Connect to EOS Utility", PTP/IP (eos.rs). The camera runs
//! one or the other; `connect` asks it which. Both give the same errors and
//! units (fractions of the picture, degrees).

use serde::Serialize;

use crate::ccapi::{self, ExposureSetting};
use crate::{eos, ptpip};

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "detail")]
pub enum CameraError {
    /// No answer: wrong address, camera asleep, or Wi-Fi dropped.
    Unreachable(String),
    /// The camera's live view is off; starting it again fixes this.
    LiveViewOff,
    /// "Device busy" or "During shooting or recording": the camera can't
    /// answer this instant, e.g. just after live view starts or while a photo
    /// is taken. Asking again shortly fixes it.
    Busy,
    /// The camera answered with an error.
    Refused(String),
    /// The camera turned down pairing with Groundglass (EOS Utility mode).
    PairingRefused,
}

pub type Result<T> = std::result::Result<T, CameraError>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraInfo {
    pub model: String,
    pub firmware: String,
    /// Live view magnifies x5 and x10 (`Camera::zoom`).
    pub zoom: bool,
    /// The lens can stop down for a depth of field preview.
    pub depth_of_field_preview: bool,
}

/// The camera's electronic level, in degrees from level (-180..180).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Level {
    pub roll: f32,
    pub pitch: f32,
}

/// An AF frame as fractions of live view's picture (top-left corner and size).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct AfFrame {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub state: AfState,
}

/// What an AF frame shows.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AfState {
    Waiting,
    Focused,
    Tracking,
    Area,
}

/// What comes with live view besides the picture, read a few times a second.
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveDetails {
    pub level: Option<Level>,
    pub af_frames: Vec<AfFrame>,
    /// Luminance: how many of live view's samples fall on each level, dark to bright (256 levels).
    pub histogram: Option<Vec<u32>>,
}

#[derive(Serialize)]
pub struct SettingState {
    pub value: String,
    pub choices: Vec<String>,
}

pub enum Camera {
    Ccapi(ccapi::Camera),
    Ptp(eos::Camera),
}

impl Camera {
    /// Connects over the Wi-Fi function the camera runs. `address` is what the
    /// camera shows, e.g. `192.168.1.2:8080`. Its UPnP description (port 49152,
    /// not CCAPI's server) says which; without one, CCAPI as before.
    /// `on_pairing` runs once the camera turns out to be in EOS Utility mode,
    /// where the first connect waits for OK on the camera.
    pub fn connect(address: &str, on_pairing: impl FnOnce()) -> Result<(Self, CameraInfo)> {
        let host = host(address);
        match ptpip::device_description(host).map_or(Mode::Ccapi, |description| mode(&description)) {
            Mode::Ptp => {
                on_pairing();
                eos::Camera::connect(host).map(|(camera, info)| (Self::Ptp(camera), info))
            }
            Mode::Ccapi => ccapi::Camera::connect(address).map(|(camera, info)| (Self::Ccapi(camera), info)),
        }
    }

    pub fn start_live_view(&self, camera_display_on: bool) -> Result<()> {
        match self {
            Self::Ccapi(camera) => camera.start_live_view(camera_display_on),
            Self::Ptp(camera) => camera.start_live_view(camera_display_on),
        }
    }

    pub fn stop_live_view(&self) -> Result<()> {
        match self {
            Self::Ccapi(camera) => camera.stop_live_view(),
            Self::Ptp(camera) => camera.stop_live_view(),
        }
    }

    /// The newest live view frame, as JPEG bytes.
    pub fn frame(&self) -> Result<Vec<u8>> {
        match self {
            Self::Ccapi(camera) => camera.frame(),
            Self::Ptp(camera) => camera.frame(),
        }
    }

    /// Takes a photo without autofocus from the shutter: focus stays on AF-ON
    /// (back-button focus), as a camera with focus split from the shutter
    /// expects. Over PTP the camera applies its own shutter-button AF setting,
    /// which with back-button focus is the same.
    pub fn shoot(&self) -> Result<()> {
        match self {
            Self::Ccapi(camera) => camera.shoot(false),
            Self::Ptp(camera) => camera.shoot(),
        }
    }

    /// Back-button focus: `true` on press, `false` on release.
    pub fn autofocus(&self, held: bool) -> Result<()> {
        match self {
            Self::Ccapi(camera) => camera.autofocus(held),
            Self::Ptp(camera) => camera.autofocus(held),
        }
    }

    pub fn exposure_setting(&self, setting: ExposureSetting) -> Result<SettingState> {
        match self {
            Self::Ccapi(camera) => camera.exposure_setting(setting),
            Self::Ptp(camera) => camera.exposure_setting(setting),
        }
    }

    pub fn set_exposure_setting(&self, setting: ExposureSetting, value: &str) -> Result<()> {
        match self {
            Self::Ccapi(camera) => camera.set_exposure_setting(setting, value),
            Self::Ptp(camera) => camera.set_exposure_setting(setting, value),
        }
    }

    pub fn live_details(&self) -> Result<LiveDetails> {
        match self {
            Self::Ccapi(camera) => camera.live_details(),
            Self::Ptp(camera) => Ok(camera.live_details()),
        }
    }

    /// Moves the AF point to (`x`, `y`), fractions across live view's picture.
    pub fn set_focus_point(&self, x: f64, y: f64) -> Result<()> {
        match self {
            Self::Ccapi(camera) => camera.set_focus_point(x, y),
            Self::Ptp(camera) => camera.set_focus_point(x, y),
        }
    }

    /// Magnifies live view 1 (whole picture), 5 or 10 times, where `CameraInfo::zoom` says it can.
    pub fn zoom(&self, level: u32) -> Result<()> {
        match self {
            Self::Ccapi(_) => Err(CameraError::Refused("live view can't magnify over CCAPI".into())),
            Self::Ptp(camera) => camera.zoom(level),
        }
    }

    /// Depth of field preview on or off, where `CameraInfo::depth_of_field_preview` says it can.
    pub fn depth_of_field_preview(&self, on: bool) -> Result<()> {
        match self {
            Self::Ccapi(_) => Err(CameraError::Refused("no depth of field preview over CCAPI".into())),
            Self::Ptp(camera) => camera.depth_of_field_preview(on),
        }
    }

    /// The battery's charge in percent, None when the camera doesn't say.
    pub fn battery(&self) -> Result<Option<u8>> {
        match self {
            Self::Ccapi(camera) => camera.battery(),
            Self::Ptp(camera) => camera.battery(),
        }
    }

    /// Whether the camera needs a stop on exit. CCAPI: only while live, since
    /// after a long idle the camera may have closed the session and only
    /// `connect` may open one. PTP: while the session is open.
    pub fn is_live(&self) -> bool {
        match self {
            Self::Ccapi(camera) => camera.is_live(),
            Self::Ptp(camera) => camera.is_open(),
        }
    }
}

/// The camera's Wi-Fi function.
#[derive(Debug, PartialEq)]
enum Mode {
    Ccapi,
    /// "Connect to EOS Utility": PTP/IP.
    Ptp,
}

/// From the camera's UPnP description: CCAPI lists its address
/// (`X_accessURL`), EOS Utility mode its service. CCAPI whenever in doubt.
fn mode(description: &[u8]) -> Mode {
    let text = String::from_utf8_lossy(description);
    if !text.contains("X_accessURL") && text.contains("ICPO-WFTEOSSystemService") {
        Mode::Ptp
    } else {
        Mode::Ccapi
    }
}

/// `192.168.1.2` from `192.168.1.2`, `192.168.1.2:8080` or `http://192.168.1.2:8080/ccapi`.
fn host(address: &str) -> &str {
    let host = address.trim().trim_start_matches("http://");
    let host = host.split('/').next().unwrap_or(host);
    host.split(':').next().unwrap_or(host)
}

#[cfg(test)]
mod tests {
    use super::{Mode, host, mode};

    #[test]
    fn the_description_names_the_wifi_function() {
        // As the R6 Mark III lists itself in "Connect to EOS Utility" (2026-10-29).
        let eos = b"<deviceType>urn:schemas-upnp-org:device:Basic:1</deviceType>\
                    <serviceType>urn:schemas-canon-com:service:ICPO-WFTEOSSystemService:1</serviceType>";
        assert_eq!(mode(eos), Mode::Ptp);
        let ccapi = br#"<ns:X_accessURL xmlns:ns="urn:schemas-canon-com:schema-imink">http://192.168.1.2:8080/ccapi</ns:X_accessURL>"#;
        assert_eq!(mode(ccapi), Mode::Ccapi);
        // A CCAPI address wins: CCAPI as before whenever it is listed.
        assert_eq!(mode(&[&ccapi[..], &eos[..]].concat()), Mode::Ccapi);
        assert_eq!(mode(b""), Mode::Ccapi);
    }

    #[test]
    fn host_accepts_what_the_camera_shows() {
        assert_eq!(host("192.168.1.2"), "192.168.1.2");
        assert_eq!(host("192.168.1.2:8080"), "192.168.1.2");
        assert_eq!(host(" http://192.168.1.2:8080/ccapi "), "192.168.1.2");
    }
}

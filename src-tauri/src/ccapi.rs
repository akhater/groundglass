//! Canon CCAPI over Wi-Fi: the few calls Groundglass needs.
//!
//! Checked against an EOS R6 Mark III, firmware 1.1.0. That camera answers
//! `GET /ccapi` with "No list of APIs", so nothing here relies on the list.
//! The paths follow Canon's CCAPI reference; the open-eos-control project
//! (Apache-2.0, `CcapiClient.kt`) was read for the payload shapes.
//!
//! **A session must open with `GET /ccapi`.** On the R6 Mark III (firmware
//! 1.1.0), a new session opened by any other request (tested:
//! `GET /ccapi/ver100/deviceinformation`) leaves the request unanswered and
//! the camera stops with Err 70 until it is switched off. Measured 2026-10-27:
//! four crashes that way, none once `GET /ccapi` came first. So `connect`
//! always sends it first, and nothing else talks to a camera before `connect`.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use ureq::Agent;

use crate::camera::{AfFrame, AfState, Battery, CameraError, CameraInfo, Level, LiveDetails, Result, SettingState};

/// Largest live view frame we accept. Medium frames measured 35-80 KB.
const MAX_FRAME_BYTES: u64 = 4 * 1024 * 1024;
/// After a start, the camera answers "Live view not started" for a moment.
const FIRST_FRAME_WAIT: Duration = Duration::from_secs(3);
/// Waits before asking again after "Device busy", as open-eos-control does
/// (`JPEG_FRAME_BUSY_RETRY_DELAYS_MILLIS` in `CcapiClient.kt`, 089e71a3).
const BUSY_RETRY_MS: [u64; 2] = [50, 100];

/// The exposure settings the app shows. Paths as open-eos-control uses them
/// (`PRIMARY_SETTING_ENDPOINTS` in `CcapiClient.kt`, 089e71a3).
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExposureSetting {
    Iso,
    Av,
    Tv,
}

impl ExposureSetting {
    fn path(self) -> &'static str {
        match self {
            Self::Iso => "/shooting/settings/iso",
            Self::Av => "/shooting/settings/av",
            Self::Tv => "/shooting/settings/tv",
        }
    }
}

pub struct Camera {
    agent: Agent,
    /// `http://<host>:<port>/ccapi`
    root: String,
    /// True between a started live view and its stop.
    live: AtomicBool,
    /// The user's Picture Style while live view has it switched to Neutral.
    user_style: Mutex<Option<String>>,
}

/// Live view runs in this Picture Style: the conversion to LUT input is measured on it.
const LIVE_STYLE: &str = "neutral";
const STYLE_PATH: &str = "/shooting/settings/picturestyle";

impl Camera {
    /// Opens a session. `address` is what the camera shows, e.g. `192.168.1.2:8080`.
    pub fn connect(address: &str) -> Result<(Self, CameraInfo)> {
        let agent: Agent = Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(3)))
            .timeout_global(Some(Duration::from_secs(10)))
            .http_status_as_error(false)
            .build()
            .into();
        let camera = Self {
            agent,
            root: root_url(address),
            live: AtomicBool::new(false),
            user_style: Mutex::new(None),
        };
        camera.request("GET", "", None)?; // must be first: see the module notes
        let body = camera.get("/deviceinformation")?;
        let json: serde_json::Value =
            serde_json::from_slice(&body).map_err(|e| CameraError::Refused(e.to_string()))?;
        let text = |key: &str| json[key].as_str().unwrap_or("?").to_string();
        // The R6 Mark III's CCAPI has no path for either (see BACKLOG, "Live view zoom").
        let info = CameraInfo {
            model: text("productname"),
            firmware: text("firmwareversion"),
            zoom: false,
            depth_of_field_preview: false,
        };
        Ok((camera, info))
    }

    /// Starts live view in Picture Style Neutral, returning once the first frame is ready.
    /// Stops first, so a stale session on the camera can never block a start.
    pub fn start_live_view(&self, camera_display_on: bool) -> Result<()> {
        let _ = self.stop_live_view();
        // Without Neutral, looks are off in colour but live view still works: carry on.
        let _ = self.switch_to_live_style();
        let display = if camera_display_on { "on" } else { "off" };
        self.post(
            "/shooting/liveview",
            &format!(r#"{{"liveviewsize":"medium","cameradisplay":"{display}"}}"#),
        )?;
        self.live.store(true, Ordering::Relaxed);
        let started = Instant::now();
        loop {
            match self.frame() {
                Err(CameraError::LiveViewOff | CameraError::Busy) if started.elapsed() < FIRST_FRAME_WAIT => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                other => return other.map(|_| ()),
            }
        }
    }

    /// Turns live view off, the camera's screen back on, and puts back the
    /// user's Picture Style. The camera accepts this even when live view is already off.
    pub fn stop_live_view(&self) -> Result<()> {
        self.post("/shooting/liveview", r#"{"liveviewsize":"off","cameradisplay":"on"}"#)?;
        self.live.store(false, Ordering::Relaxed);
        self.restore_user_style()
    }

    fn switch_to_live_style(&self) -> Result<()> {
        let mut user_style = self.user_style.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if user_style.is_some() {
            return Ok(()); // already switched
        }
        let body = self.get(STYLE_PATH)?;
        let current = serde_json::from_slice::<serde_json::Value>(&body)
            .ok()
            .and_then(|json| json["value"].as_str().map(str::to_string))
            .ok_or_else(|| CameraError::Refused("no Picture Style".into()))?;
        if current != LIVE_STYLE {
            self.put(STYLE_PATH, &format!(r#"{{"value":"{LIVE_STYLE}"}}"#))?;
            *user_style = Some(current);
        }
        Ok(())
    }

    fn restore_user_style(&self) -> Result<()> {
        let mut user_style = self.user_style.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(style) = user_style.as_deref() {
            self.put(STYLE_PATH, &format!(r#"{{"value":"{style}"}}"#))?;
            *user_style = None;
        }
        Ok(())
    }

    /// Takes a photo, like a full press of the shutter button, focusing first when `autofocus`.
    /// Payload as open-eos-control sends it (`CcapiClient.kt`, 089e71a3).
    pub fn shoot(&self, autofocus: bool) -> Result<()> {
        self.post("/shooting/control/shutterbutton", &format!(r#"{{"af":{autofocus}}}"#))
    }

    /// Focuses while held, like a back-button AF-ON: `true` on press, `false` on release.
    pub fn autofocus(&self, held: bool) -> Result<()> {
        let action = if held { "start" } else { "stop" };
        self.post("/shooting/control/af", &format!(r#"{{"action":"{action}"}}"#))
    }

    /// An exposure setting's current value and the values the camera accepts now
    /// (empty when the shooting mode sets it, e.g. aperture in Tv).
    pub fn exposure_setting(&self, setting: ExposureSetting) -> Result<SettingState> {
        let body = self.get(setting.path())?;
        let json: serde_json::Value =
            serde_json::from_slice(&body).map_err(|e| CameraError::Refused(e.to_string()))?;
        let text = |v: &serde_json::Value| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string());
        Ok(SettingState {
            value: text(&json["value"]),
            choices: json["ability"].as_array().map(|a| a.iter().map(text).collect()).unwrap_or_default(),
        })
    }

    pub fn set_exposure_setting(&self, setting: ExposureSetting, value: &str) -> Result<()> {
        let json = serde_json::json!({ "value": value }).to_string();
        self.put(setting.path(), &json)
    }

    /// Any read under /ccapi, e.g. `/ver110/event/polling?continue=off`: for
    /// headless research into what the camera reports. Reads only.
    pub fn read_raw(&self, path: &str) -> Result<Vec<u8>> {
        self.request("GET", path, None)
    }

    /// The level, AF frames and histogram while live view runs, from live view's details
    /// (`flipdetail?kind=info`), which carry them on every request. (The event
    /// poll lists the level only when it changes, shared by every client.)
    pub fn live_details(&self) -> Result<LiveDetails> {
        let details = self.get("/shooting/liveview/flipdetail?kind=info")?;
        Ok(LiveDetails {
            level: level_from(&details),
            af_frames: af_frames_from(&details),
            histogram: histogram_from(&details),
        })
    }

    /// Moves the AF point to (`x`, `y`), fractions across live view's
    /// picture, as touching the camera's screen does.
    pub fn set_focus_point(&self, x: f64, y: f64) -> Result<()> {
        let details = self.get("/shooting/liveview/flipdetail?kind=info")?;
        let (x, y) = focus_point(&details, x, y)
            .ok_or_else(|| CameraError::Refused("the camera gave no live view picture size".into()))?;
        self.put("/shooting/liveview/afframeposition", &format!(r#"{{"positionx":{x},"positiony":{y}}}"#))
    }

    /// The battery's charge in percent: from `batterylist` (ver110, in percent)
    /// where the camera has it, else from `battery` (ver100, in quarters).
    pub fn battery(&self) -> Result<Option<Battery>> {
        let body = match self.request("GET", "/ver110/devicestatus/batterylist", None) {
            Err(CameraError::Refused(_)) => self.get("/devicestatus/battery")?,
            other => other?,
        };
        Ok(battery_from(&body))
    }

    pub fn is_live(&self) -> bool {
        self.live.load(Ordering::Relaxed)
    }

    /// The newest live view frame, as JPEG bytes. Asked for one at a time: the
    /// camera's push stream (`/shooting/liveview/scroll`) was tried and measured
    /// the same lag, about 0.3 s glass to PC (2026-10-28), so it isn't used.
    pub fn frame(&self) -> Result<Vec<u8>> {
        for wait in BUSY_RETRY_MS {
            match self.get("/shooting/liveview/flip") {
                Err(CameraError::Busy) => std::thread::sleep(Duration::from_millis(wait)),
                other => return other,
            }
        }
        self.get("/shooting/liveview/flip")
    }

    fn get(&self, path: &str) -> Result<Vec<u8>> {
        self.request("GET", &format!("/ver100{path}"), None)
    }

    fn post(&self, path: &str, json: &str) -> Result<()> {
        self.request("POST", &format!("/ver100{path}"), Some(json)).map(|_| ())
    }

    fn put(&self, path: &str, json: &str) -> Result<()> {
        self.request("PUT", &format!("/ver100{path}"), Some(json)).map(|_| ())
    }

    /// `path` is below `/ccapi`. A body is sent with POST or PUT.
    fn request(&self, method: &str, path: &str, json: Option<&str>) -> Result<Vec<u8>> {
        let started = Instant::now();
        let url = format!("{}{path}", self.root);
        let sent = match (method, json) {
            ("PUT", Some(json)) => self.agent.put(url).header("Content-Type", "application/json").send(json),
            (_, Some(json)) => self.agent.post(url).header("Content-Type", "application/json").send(json),
            (_, None) => self.agent.get(url).call(),
        };
        let result = sent.map_err(unreachable).and_then(read);
        log(method, path, json.unwrap_or(""), started, &result);
        result
    }
}

/// Live view's details arrive in a binary frame around the JSON: read the JSON
/// object from its first `{` and ignore what follows.
fn details_json(details: &[u8]) -> Option<serde_json::Value> {
    let start = details.iter().position(|&b| b == b'{')?;
    serde_json::Deserializer::from_slice(&details[start..]).into_iter().next()?.ok()
}

/// Sensor pixels for a point given as fractions across live view's picture,
/// which covers `liveviewdata.image` on the sensor (a part of it when zoomed).
fn focus_point(details: &[u8], x: f64, y: f64) -> Option<(u32, u32)> {
    let json = details_json(details)?;
    let image = &json["liveviewdata"]["image"];
    let along = |start: &str, length: &str, fraction: f64| -> Option<u32> {
        let start = image[start].as_f64()?;
        let length = image[length].as_f64()?;
        Some((start + fraction.clamp(0.0, 1.0) * length).round() as u32)
    };
    Some((along("positionx", "positionwidth", x)?, along("positiony", "positionheight", y)?))
}

/// The camera gives frames in sensor pixels; the picture covers `image` on it.
fn af_frames_from(details: &[u8]) -> Vec<AfFrame> {
    let Some(json) = details_json(details) else { return Vec::new() };
    let data = &json["liveviewdata"];
    let image = &data["image"];
    let (Some(left), Some(top), Some(width), Some(height)) = (
        image["positionx"].as_f64(),
        image["positiony"].as_f64(),
        image["positionwidth"].as_f64(),
        image["positionheight"].as_f64(),
    ) else {
        return Vec::new();
    };
    let frames = data["afframe"].as_array().map(Vec::as_slice).unwrap_or_default();
    frames
        .iter()
        .filter_map(|frame| {
            Some(AfFrame {
                x: ((frame["x"].as_f64()? - left) / width) as f32,
                y: ((frame["y"].as_f64()? - top) / height) as f32,
                width: (frame["width"].as_f64()? / width) as f32,
                height: (frame["height"].as_f64()? / height) as f32,
                state: AfState::from_status(frame["status"].as_u64()),
            })
        })
        .collect()
}

/// The luminance histogram: the first of `liveviewdata.histogram`'s arrays.
fn histogram_from(details: &[u8]) -> Option<Vec<u32>> {
    let json = details_json(details)?;
    let counts = json["liveviewdata"]["histogram"].get(0)?.as_array()?;
    counts.iter().map(|count| count.as_u64().map(|count| count as u32)).collect()
}

/// The battery's charge, from ver110 `batterylist` (its first battery, the
/// level in percent) or ver100 `battery` (a word for the level: a step).
fn battery_from(body: &[u8]) -> Option<Battery> {
    let json: serde_json::Value = serde_json::from_slice(body).ok()?;
    let battery = json.get("batterylist").map_or(Some(&json), |list| list.get(0))?;
    let level = &battery["level"];
    let (percent, exact) = match level.as_str() {
        Some("full") => (100, false),
        Some("high") => (75, false),
        Some("half") => (50, false),
        Some("quarter") => (25, false),
        Some("low") => (10, false),
        Some(text) => (text.parse().ok()?, true),
        None => (level.as_u64()?, true),
    };
    let percent = u8::try_from(percent).ok().filter(|&percent| percent <= 100)?;
    Some(Battery { percent, exact })
}

fn level_from(details: &[u8]) -> Option<Level> {
    let json = details_json(details)?;
    let angle = &json["angleinformation"];
    // Hundredths of a degree, 0..36000: 35100 is 9 degrees below level.
    let degrees = |hundredths: f64| {
        let d = hundredths / 100.0;
        (if d > 180.0 { d - 360.0 } else { d }) as f32
    };
    Some(Level {
        roll: degrees(angle["rolling"].as_f64()?),
        pitch: degrees(angle["pitching"].as_f64()?),
    })
}

/// Every request on stderr, except successful frames (counted) and live view
/// details (several a second).
fn log<T>(method: &str, path: &str, body: &str, started: Instant, result: &Result<T>) {
    static FRAMES: AtomicU64 = AtomicU64::new(0);
    let ms = started.elapsed().as_millis();
    if result.is_ok() && path.contains("/flipdetail") {
        return;
    }
    if result.is_ok() && path.ends_with("/flip") {
        let frames = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
        if frames.is_multiple_of(100) {
            eprintln!("[ccapi] {frames} frames so far (last took {ms} ms)");
        }
        return;
    }
    let outcome = match result {
        Ok(_) => "ok".to_string(),
        Err(error) => format!("{error:?}"),
    };
    eprintln!("[ccapi] {method} {path} {body} -> {outcome} in {ms} ms");
}

fn read(mut response: ureq::http::Response<ureq::Body>) -> Result<Vec<u8>> {
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_FRAME_BYTES)
        .read_to_vec()
        .map_err(unreachable)?;
    match status {
        200..=299 => Ok(body),
        _ => {
            let message = serde_json::from_slice::<serde_json::Value>(&body)
                .ok()
                .and_then(|json| json["message"].as_str().map(str::to_string))
                .unwrap_or_default();
            if message.eq_ignore_ascii_case("live view not started") {
                Err(CameraError::LiveViewOff)
            } else if message.eq_ignore_ascii_case("device busy")
                // While a photo is being taken (a slow shutter makes it long).
                || message.eq_ignore_ascii_case("during shooting or recording")
            {
                Err(CameraError::Busy)
            } else {
                Err(CameraError::Refused(format!("HTTP {status} {message}")))
            }
        }
    }
}

fn unreachable(error: ureq::Error) -> CameraError {
    CameraError::Unreachable(error.to_string())
}

/// `192.168.1.2`, `192.168.1.2:8080` or `http://192.168.1.2:8080/` all work.
fn root_url(address: &str) -> String {
    let host = address.trim().trim_start_matches("http://").trim_end_matches('/');
    let host = host.split('/').next().unwrap_or(host);
    let port = if host.contains(':') { "" } else { ":8080" };
    format!("http://{host}{port}/ccapi")
}

#[cfg(test)]
mod tests {
    use super::{AfFrame, AfState, Battery, Level, af_frames_from, battery_from, focus_point, histogram_from, level_from, root_url};

    #[test]
    fn root_url_accepts_what_the_camera_shows() {
        let want = "http://192.168.1.2:8080/ccapi";
        assert_eq!(root_url("192.168.1.2"), want);
        assert_eq!(root_url("192.168.1.2:8080"), want);
        assert_eq!(root_url(" http://192.168.1.2:8080/ "), want);
        assert_eq!(root_url("http://192.168.1.2:8080/ccapi"), want);
    }

    #[test]
    fn level_reads_hundredths_of_a_degree_around_zero() {
        // Framed as the R6 Mark III sends it (2026-10-28), lying almost flat.
        let mut details = vec![0xff, 0x00, 0x02, 0x00, 0x00, 0x01, 0x00];
        details.extend_from_slice(
            br#"{"liveviewdata":{"histogram":[[0]]},"angleinformation":{"cameraposture":0,"rolling":100,"pitching":35100}}"#,
        );
        details.extend_from_slice(&[0xff, 0xff]);
        assert_eq!(level_from(&details), Some(Level { roll: 1.0, pitch: -9.0 }));
    }

    #[test]
    fn focus_point_maps_the_picture_onto_the_sensor() {
        // As the R6 Mark III reports live view's picture: the whole sensor.
        let details = br#"{"liveviewdata":{"image":{"positionx":0,"positiony":0,"positionheight":4640,"positionwidth":6960,"sizex":6960,"sizey":4640}}}"#;
        assert_eq!(focus_point(details, 0.5, 0.5), Some((3480, 2320)));
        assert_eq!(focus_point(details, 0.0, 1.0), Some((0, 4640)));
        // Zoomed in: the picture is a part of the sensor.
        let zoomed = br#"{"liveviewdata":{"image":{"positionx":2784,"positiony":1856,"positionheight":928,"positionwidth":1392}}}"#;
        assert_eq!(focus_point(zoomed, 0.5, 0.5), Some((3480, 2320)));
        assert_eq!(focus_point(b"{}", 0.5, 0.5), None);
    }

    #[test]
    fn af_frames_are_fractions_of_the_picture() {
        // Statuses as the R6 Mark III reported them (2026-10-28), simplified sizes.
        let details = br#"{"liveviewdata":{"afframe":[{"status":17,"select":1,"x":3480,"y":0,"width":696,"height":464},{"status":48,"select":0,"x":0,"y":2320,"width":696,"height":464},{"status":32,"select":1,"x":0,"y":0,"width":6960,"height":4640},{"status":20,"select":1,"x":0,"y":0,"width":696,"height":464}],"image":{"positionx":0,"positiony":0,"positionheight":4640,"positionwidth":6960}}}"#;
        let frame = |x, y, width, height, state| AfFrame { x, y, width, height, state };
        assert_eq!(
            af_frames_from(details),
            vec![
                frame(0.5, 0.0, 0.1, 0.1, AfState::Focused),
                frame(0.0, 0.5, 0.1, 0.1, AfState::Waiting),
                frame(0.0, 0.0, 1.0, 1.0, AfState::Area),
                frame(0.0, 0.0, 0.1, 0.1, AfState::Tracking),
            ]
        );
        assert_eq!(af_frames_from(br#"{"liveviewdata":{"afframe":[]}}"#), vec![]);
    }

    #[test]
    fn the_histogram_is_the_first_of_its_arrays() {
        // Shaped as the R6 Mark III sends it (2026-10-28), cut short.
        let details = br#"{"liveviewdata":{"histogram":[[472481,67515,4,0],[1,2,3,4]]}}"#;
        assert_eq!(histogram_from(details), Some(vec![472481, 67515, 4, 0]));
        assert_eq!(histogram_from(br#"{"liveviewdata":{"histogram":[]}}"#), None);
        assert_eq!(histogram_from(br#"{"liveviewdata":{"histogram":[["x"]]}}"#), None);
        assert_eq!(histogram_from(br#"{"liveviewdata":{}}"#), None);
    }

    #[test]
    fn level_is_none_without_an_angle() {
        assert_eq!(level_from(br#"{"liveviewdata":{}}"#), None);
        assert_eq!(level_from(b"not json"), None);
    }

    #[test]
    fn battery_reads_the_first_battery_in_percent() {
        let exact = |percent| Some(Battery { percent, exact: true });
        let step = |percent| Some(Battery { percent, exact: false });
        // ver110 batterylist: the level in percent, as a string.
        let list = br#"{"batterylist":[{"position":"camera","name":"LP-E6P","kind":"battery","level":"87","quality":"good"}]}"#;
        assert_eq!(battery_from(list), exact(87));
        assert_eq!(battery_from(br#"{"level":90}"#), exact(90));
        // ver100 battery: a word for the level, a step rather than a percent.
        assert_eq!(battery_from(br#"{"name":"LP-E6P","kind":"battery","level":"full","quality":"good"}"#), step(100));
        assert_eq!(battery_from(br#"{"level":"high"}"#), step(75));
        assert_eq!(battery_from(br#"{"level":"half"}"#), step(50));
        assert_eq!(battery_from(br#"{"level":"quarter"}"#), step(25));
        assert_eq!(battery_from(br#"{"level":"low"}"#), step(10));
    }

    #[test]
    fn battery_is_none_without_a_known_level() {
        assert_eq!(battery_from(br#"{"level":"unknown"}"#), None);
        assert_eq!(battery_from(br#"{"level":"250"}"#), None);
        assert_eq!(battery_from(br#"{"batterylist":[]}"#), None);
        assert_eq!(battery_from(b"not json"), None);
    }
}

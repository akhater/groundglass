//! The camera in its "Connect to EOS Utility" Wi-Fi function: a PTP/IP
//! session (ptpip.rs) presented as the app's camera, with the same errors and
//! units as CCAPI. One transaction at a time: each call holds the session for
//! one exchange, so a command waits one frame at most. No threads, no queues.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::camera::{AfFrame, AfState, CameraError, CameraInfo, Level, LiveDetails, Result, SettingState};
use crate::canon_codes::{self, PICTURE_STYLE_NEUTRAL};
use crate::ccapi::ExposureSetting;
use crate::ptpip::{self, Session};

/// The first connect waits this long for the pairing to be confirmed on the camera.
const PAIRING_WAIT: Duration = Duration::from_secs(30);
/// Once the session is open, no answer within this means the camera is gone.
const ANSWER_WAIT: Duration = Duration::from_secs(3);
/// The camera expects its reports to be read now and then, as EOS Utility does.
const EVENTS_EVERY: Duration = Duration::from_millis(250);
/// While the camera has no new frame, ask again this often, this many times.
const NOT_READY_WAIT: Duration = Duration::from_millis(20);
const NOT_READY_TRIES: u32 = 5;
/// The AF point is drawn this part of the sensor wide and high, about the
/// size of the camera's own frame (PTP gives a point, not a size).
const AF_FRAME_PART: f64 = 0.1;
/// Canon's Picture Style property.
const PICTURE_STYLE: u32 = 0xd110;

pub struct Camera {
    /// None once closed: a closed session never reopens, `connect` makes a new camera.
    link: Mutex<Option<Link>>,
    latest: Mutex<Latest>,
}

struct Link {
    session: Session,
    /// When the camera's reports were last read.
    events_at: Instant,
    /// The user's Picture Style while live view has it switched to Neutral.
    user_style: Option<u32>,
}

impl Camera {
    /// Pairs (the first time, the camera asks to confirm on its screen) and opens a session.
    pub fn connect(host: &str) -> Result<(Self, CameraInfo)> {
        let mut session = Session::open(host, PAIRING_WAIT).map_err(camera_error)?;
        session.set_timeout(ANSWER_WAIT).map_err(camera_error)?;
        let device = session.device_info().map_err(camera_error)?;
        let info = CameraInfo { model: device.model, firmware: firmware(&device.version).to_string() };
        let link = Link { session, events_at: Instant::now(), user_style: None };
        Ok((Self { link: Mutex::new(Some(link)), latest: Mutex::default() }, info))
    }

    /// Starts live view in Picture Style Neutral, to the camera's screen too
    /// when `camera_display_on`.
    pub fn start_live_view(&self, camera_display_on: bool) -> Result<()> {
        self.exchange(|link| {
            link.session.start_live_view()?; // its reports carry the Picture Style
            // Without Neutral, looks are off in colour but live view still works: carry on.
            if link.user_style.is_none()
                && let Some(style) = style_to_restore(link.session.properties().value(PICTURE_STYLE))
                && link.session.set_property(PICTURE_STYLE, PICTURE_STYLE_NEUTRAL).is_ok()
            {
                eprintln!("[eos] Picture Style {} -> Neutral", canon_codes::picture_style(style));
                link.user_style = Some(style);
            }
            if camera_display_on {
                // Without it live view still works, the camera's screen off: carry on.
                let _ = link.session.live_view_output(true);
            }
            Ok(())
        })
    }

    /// The user's Picture Style back, live view back to the camera's screen,
    /// then the session ends, so stop, exit and hiding the app never leave a
    /// session open.
    pub fn stop_live_view(&self) -> Result<()> {
        let Some(mut link) = lock(&self.link).take() else { return Ok(()) };
        let style = link.user_style.map_or(Ok(()), |style| link.session.set_property(PICTURE_STYLE, style));
        let output = link.session.stop_live_view();
        let closed = link.session.close();
        style.and(output).and(closed).map_err(camera_error)
    }

    /// The newest frame, as JPEG bytes. What comes with it is kept for
    /// `live_details`, and the camera's reports are read every quarter second.
    pub fn frame(&self) -> Result<Vec<u8>> {
        for _ in 0..NOT_READY_TRIES {
            if let Some(jpeg) = self.try_frame()? {
                return Ok(jpeg);
            }
            std::thread::sleep(NOT_READY_WAIT); // the session is free meanwhile
        }
        Err(CameraError::Busy)
    }

    fn try_frame(&self) -> Result<Option<Vec<u8>>> {
        let started = Instant::now();
        let data = self.exchange(|link| {
            if events_due(link.events_at, Instant::now()) {
                let reading = Instant::now();
                link.session.events()?;
                log_slow("reports", reading);
                link.events_at = Instant::now();
            }
            link.session.viewfinder_data()
        })?;
        log_frame(data.is_some(), started);
        let Some(data) = data else { return Ok(None) };
        lock(&self.latest).take_in(&data);
        Ok(ptpip::viewfinder_jpeg(&data).map(<[u8]>::to_vec))
    }

    pub fn shoot(&self) -> Result<()> {
        self.exchange(|link| link.session.shoot())
    }

    pub fn autofocus(&self, held: bool) -> Result<()> {
        self.exchange(|link| link.session.autofocus(held))
    }

    /// From what the camera last reported (read every quarter second with the
    /// frames): no camera traffic of its own.
    pub fn exposure_setting(&self, setting: ExposureSetting) -> Result<SettingState> {
        let (property, label) = exposure_property(setting);
        let link = lock(&self.link);
        let properties = link.as_ref().ok_or_else(not_connected)?.session.properties();
        let value = properties
            .value(property)
            .ok_or_else(|| CameraError::Refused("the camera hasn't reported this setting".into()))?;
        Ok(setting_state(label, value, properties.allowed(property)))
    }

    /// `value` as `exposure_setting` gives it, e.g. "3200", "f4.0", "1/125".
    pub fn set_exposure_setting(&self, setting: ExposureSetting, value: &str) -> Result<()> {
        let (property, label) = exposure_property(setting);
        let code = {
            let link = lock(&self.link);
            let allowed = link.as_ref().ok_or_else(not_connected)?.session.properties().allowed(property);
            setting_code(label, allowed, value).ok_or_else(|| CameraError::Refused(format!("{value} isn't allowed now")))?
        };
        self.exchange(|link| link.session.set_property(property, code))
    }

    /// From what came with the newest frames: no camera traffic.
    pub fn live_details(&self) -> LiveDetails {
        let latest = lock(&self.latest);
        LiveDetails { level: latest.level, af_frames: latest.af_frames() }
    }

    /// Moves the AF point to (`x`, `y`), fractions across live view's picture,
    /// as touching the camera's screen does.
    pub fn set_focus_point(&self, x: f64, y: f64) -> Result<()> {
        let picture = lock(&self.latest)
            .picture()
            .ok_or_else(|| CameraError::Refused("the camera gave no live view picture size".into()))?;
        let (x, y) = sensor_point(picture, x, y);
        self.exchange(|link| link.session.touch_af(x, y))
    }

    pub fn is_open(&self) -> bool {
        lock(&self.link).is_some()
    }

    /// Runs one exchange on the open session. A broken connection is let go,
    /// so nothing reads a late answer to a request that timed out.
    fn exchange<T>(&self, run: impl FnOnce(&mut Link) -> std::result::Result<T, ptpip::Error>) -> Result<T> {
        let mut link = lock(&self.link);
        let result = run(link.as_mut().ok_or_else(not_connected)?);
        if let Err(error) = &result {
            eprintln!("[eos] {error:?}");
        }
        if let Err(ptpip::Error::Connection(_) | ptpip::Error::Protocol(_)) = result {
            *link = None;
        }
        result.map_err(camera_error)
    }
}

/// What came with the newest frames, for `live_details` and taps.
#[derive(Default)]
struct Latest {
    level: Option<Level>,
    /// Width and height in pixels.
    sensor: Option<[u32; 2]>,
    /// The top-left corner of the magnify area around the AF point.
    af_corner: Option<[u32; 2]>,
    /// The magnify area: x, y, width, height.
    zoom_area: Option<[u32; 4]>,
}

impl Latest {
    /// Takes in a GetViewFinderData reply. What it lacks keeps its last value.
    fn take_in(&mut self, data: &[u8]) {
        self.level = ptpip::viewfinder_level(data).or(self.level);
        self.sensor = ptpip::viewfinder_sensor(data).or(self.sensor);
        self.af_corner = ptpip::viewfinder_af_corner(data).or(self.af_corner);
        self.zoom_area = ptpip::viewfinder_zoom_area(data).or(self.zoom_area);
    }

    /// What live view's picture covers on the sensor: all of it.
    fn picture(&self) -> Option<Area> {
        let [width, height] = self.sensor?;
        Some(Area { x: 0.0, y: 0.0, width: width as f64, height: height as f64 })
    }

    /// The AF point, in the middle of its magnify area, as a frame on the
    /// picture. PTP gives no AF state yet: always "waiting".
    fn af_frames(&self) -> Vec<AfFrame> {
        let (Some(picture), Some([sensor_width, sensor_height]), Some([left, top])) =
            (self.picture(), self.sensor, self.af_corner)
        else {
            return Vec::new();
        };
        let [width, height] = match self.zoom_area {
            Some([_, _, width, height]) => [width, height],
            None => [sensor_width / 5, sensor_height / 5],
        };
        let size = [sensor_width as f64 * AF_FRAME_PART, sensor_height as f64 * AF_FRAME_PART];
        let centre = [left as f64 + width as f64 / 2.0, top as f64 + height as f64 / 2.0];
        vec![AfFrame {
            x: ((centre[0] - size[0] / 2.0 - picture.x) / picture.width) as f32,
            y: ((centre[1] - size[1] / 2.0 - picture.y) / picture.height) as f32,
            width: (size[0] / picture.width) as f32,
            height: (size[1] / picture.height) as f32,
            state: AfState::Waiting,
        }]
    }
}

/// A part of the sensor, in sensor pixels.
#[derive(Clone, Copy, Debug)]
struct Area {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Sensor pixels for a point given as fractions across the picture, which covers `picture`.
fn sensor_point(picture: Area, x: f64, y: f64) -> (u32, u32) {
    let along = |start: f64, length: f64, fraction: f64| (start + fraction.clamp(0.0, 1.0) * length).round() as u32;
    (along(picture.x, picture.width, x), along(picture.y, picture.height, y))
}

/// The PTP property behind each exposure setting, and how its codes read.
fn exposure_property(setting: ExposureSetting) -> (u32, fn(u32) -> String) {
    match setting {
        ExposureSetting::Iso => (0xd103, canon_codes::iso),
        ExposureSetting::Av => (0xd101, canon_codes::aperture),
        ExposureSetting::Tv => (0xd102, canon_codes::shutter_speed),
    }
}

/// A setting as CCAPI gives it: its value and the values allowed now, as labels.
fn setting_state(label: fn(u32) -> String, value: u32, allowed: &[u32]) -> SettingState {
    SettingState { value: label(value), choices: allowed.iter().map(|&code| label(code)).collect() }
}

/// The allowed code that reads as `wanted`.
fn setting_code(label: fn(u32) -> String, allowed: &[u32], wanted: &str) -> Option<u32> {
    allowed.iter().copied().find(|&code| label(code) == wanted)
}

fn not_connected() -> CameraError {
    CameraError::Unreachable("Not connected".into())
}

/// The user's Picture Style, to put back on stop, when live view switches it
/// to Neutral. None when it is Neutral already or the camera didn't report it.
fn style_to_restore(current: Option<u32>) -> Option<u32> {
    current.filter(|&style| style != PICTURE_STYLE_NEUTRAL)
}

fn events_due(read_at: Instant, now: Instant) -> bool {
    now.duration_since(read_at) >= EVENTS_EVERY
}

/// The same errors as over CCAPI, so the frame loop treats both alike.
fn camera_error(error: ptpip::Error) -> CameraError {
    match error {
        ptpip::Error::Busy => CameraError::Busy,
        ptpip::Error::Connection(detail) => CameraError::Unreachable(detail),
        ptpip::Error::PairingRefused => CameraError::PairingRefused,
        other => CameraError::Refused(other.to_string()),
    }
}

/// The R6 Mark III gives its firmware 1.1.0 as "1-1.1.0" over PTP, "1.1.0" over CCAPI.
fn firmware(version: &str) -> &str {
    version.split_once('-').map_or(version, |(_, firmware)| firmware)
}

/// Frames counted on stderr, as ccapi.rs does, with how often the camera had none ready.
fn log_frame(ready: bool, started: Instant) {
    static FRAMES: AtomicU64 = AtomicU64::new(0);
    static NOT_READY: AtomicU64 = AtomicU64::new(0);
    if !ready {
        NOT_READY.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let frames = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    if frames.is_multiple_of(100) {
        let not_ready = NOT_READY.load(Ordering::Relaxed);
        let ms = started.elapsed().as_millis();
        eprintln!("[eos] {frames} frames so far, {not_ready} not ready (last took {ms} ms)");
    }
}

fn log_slow(what: &str, started: Instant) {
    let ms = started.elapsed().as_millis();
    if ms >= 100 {
        eprintln!("[eos] {what} took {ms} ms");
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::{
        Area, EVENTS_EVERY, Latest, camera_error, events_due, firmware, sensor_point, setting_code, setting_state,
        style_to_restore,
    };
    use crate::camera::{AfFrame, AfState, CameraError, Level};
    use crate::canon_codes::{self, PICTURE_STYLE_NEUTRAL};
    use crate::ptpip::Error;
    use std::time::Instant;

    /// A GetViewFinderData reply: blocks of length, type, body.
    fn reply(blocks: &[(u32, &[u32])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (kind, words) in blocks {
            out.extend_from_slice(&(8 + 4 * words.len() as u32).to_le_bytes());
            out.extend_from_slice(&kind.to_le_bytes());
            for word in *words {
                out.extend_from_slice(&word.to_le_bytes());
            }
        }
        out
    }

    #[test]
    fn frames_keep_the_newest_level() {
        let mut latest = Latest::default();
        assert_eq!(latest.level, None);
        latest.take_in(&reply(&[(0x0e, &[6960, 4640]), (0x10, &[2, 0, 2800, 35900])]));
        assert_eq!(latest.level, Some(Level { roll: 28.0, pitch: -1.0 }));
        // A frame without the level block keeps the last level.
        latest.take_in(&reply(&[(0x0e, &[6960, 4640])]));
        assert_eq!(latest.level, Some(Level { roll: 28.0, pitch: -1.0 }));
        latest.take_in(&reply(&[(0x10, &[2, 0, 0, 0])]));
        assert_eq!(latest.level, Some(Level { roll: 0.0, pitch: 0.0 }));
    }

    #[test]
    fn the_af_point_is_the_middle_of_its_magnify_area() {
        let mut latest = Latest::default();
        assert_eq!(latest.af_frames(), vec![]);
        // As the R6 Mark III sends them with the AF point in the centre (2026-10-29).
        latest.take_in(&reply(&[(0x0e, &[6960, 4640]), (0x05, &[2784, 1856]), (0x0d, &[2784, 1856, 1392, 928])]));
        let centre = AfFrame { x: 0.45, y: 0.45, width: 0.1, height: 0.1, state: AfState::Waiting };
        assert_eq!(latest.af_frames(), vec![centre]);
        // Without block 0xd the area is a fifth of the sensor, as the camera sends it.
        let mut latest = Latest::default();
        latest.take_in(&reply(&[(0x0e, &[6960, 4640]), (0x05, &[0, 0])]));
        let corner = AfFrame { x: 0.05, y: 0.05, width: 0.1, height: 0.1, state: AfState::Waiting };
        assert_eq!(latest.af_frames(), vec![corner]);
        // No sensor size yet: nothing to place it on.
        let mut latest = Latest::default();
        latest.take_in(&reply(&[(0x05, &[2784, 1856])]));
        assert_eq!(latest.af_frames(), vec![]);
    }

    #[test]
    fn taps_land_on_the_sensor() {
        let mut latest = Latest::default();
        assert!(latest.picture().is_none());
        latest.take_in(&reply(&[(0x0e, &[6960, 4640])]));
        let picture = latest.picture().unwrap();
        assert_eq!(sensor_point(picture, 0.5, 0.5), (3480, 2320));
        assert_eq!(sensor_point(picture, 0.25, 0.75), (1740, 3480));
        // Off the picture: its nearest edge.
        assert_eq!(sensor_point(picture, -0.1, 1.2), (0, 4640));
        // A part of the sensor (as when zoomed): the same fractions of that part.
        let part = Area { x: 2784.0, y: 1856.0, width: 1392.0, height: 928.0 };
        assert_eq!(sensor_point(part, 0.5, 0.5), (3480, 2320));
    }

    #[test]
    fn exposure_reads_as_over_ccapi() {
        let iso = setting_state(canon_codes::iso, 0x70, &[0x48, 0x70, 0x78]);
        assert_eq!(iso.value, "3200");
        assert_eq!(iso.choices, ["100", "3200", "6400"]);
        let av = setting_state(canon_codes::aperture, 0x28, &[]);
        assert_eq!((av.value.as_str(), av.choices.len()), ("f4.0", 0)); // set by the mode
        assert_eq!(setting_code(canon_codes::iso, &[0x48, 0x70], "3200"), Some(0x70));
        assert_eq!(setting_code(canon_codes::shutter_speed, &[0x70, 0x78], "1/250"), Some(0x78));
        // Not allowed now: nothing to send.
        assert_eq!(setting_code(canon_codes::aperture, &[0x28], "f5.6"), None);
    }

    #[test]
    fn reports_are_read_every_quarter_second() {
        let read = Instant::now();
        assert!(!events_due(read, read));
        assert!(!events_due(read, read + EVENTS_EVERY / 2));
        assert!(events_due(read, read + EVENTS_EVERY));
    }

    #[test]
    fn errors_are_the_same_as_over_ccapi() {
        assert_eq!(camera_error(Error::Busy), CameraError::Busy);
        assert_eq!(
            camera_error(Error::Connection("timed out".into())),
            CameraError::Unreachable("timed out".into())
        );
        assert_eq!(camera_error(Error::PairingRefused), CameraError::PairingRefused);
        assert!(matches!(
            camera_error(Error::Response { operation: 0x9128, response: 0x2002 }),
            CameraError::Refused(_)
        ));
    }

    #[test]
    fn the_users_style_is_kept_unless_already_neutral() {
        assert_eq!(style_to_restore(Some(0x81)), Some(0x81)); // Standard
        assert_eq!(style_to_restore(Some(0x21)), Some(0x21)); // User Def. 1
        assert_eq!(style_to_restore(Some(PICTURE_STYLE_NEUTRAL)), None);
        // Not reported: nothing switched, nothing to put back.
        assert_eq!(style_to_restore(None), None);
    }

    #[test]
    fn firmware_reads_as_over_ccapi() {
        // The R6 Mark III's PTP version for firmware 1.1.0 (2026-10-29).
        assert_eq!(firmware("1-1.1.0"), "1.1.0");
        assert_eq!(firmware("1.1.0"), "1.1.0");
    }
}

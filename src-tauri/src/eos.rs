//! The camera in its "Connect to EOS Utility" Wi-Fi function: a PTP/IP
//! session (ptpip.rs) presented as the app's camera, with the same errors and
//! units as CCAPI. One transaction at a time: each call holds the session for
//! one exchange, so a command waits one frame at most. No threads, no queues.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::camera::{AfFrame, AfState, Battery, CameraError, CameraInfo, Level, LiveDetails, Result, SettingState};
use crate::canon_codes::{self, PICTURE_STYLE_NEUTRAL};
use crate::ccapi::ExposureSetting;
use crate::ptpip::{self, AfBox, AfZones, Session};

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
/// Canon's battery level property.
const BATTERY: u32 = 0xd111;

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
    /// Depth of field preview is on: turned off on stop.
    depth_of_field_preview: bool,
}

impl Camera {
    /// Pairs (the first time, the camera asks to confirm on its screen) and opens a session.
    pub fn connect(host: &str) -> Result<(Self, CameraInfo)> {
        let mut session = Session::open(host, PAIRING_WAIT).map_err(camera_error)?;
        session.set_timeout(ANSWER_WAIT).map_err(camera_error)?;
        let device = session.device_info().map_err(camera_error)?;
        // The R6 Mark III magnifies and previews depth of field over PTP.
        let info = CameraInfo {
            model: device.model,
            firmware: firmware(&device.version).to_string(),
            zoom: true,
            depth_of_field_preview: true,
        };
        let link = Link { session, events_at: Instant::now(), user_style: None, depth_of_field_preview: false };
        Ok((Self { link: Mutex::new(Some(link)), latest: Mutex::default() }, info))
    }

    /// Starts live view in Picture Style Neutral, to the camera's screen too
    /// when `camera_display_on`.
    pub fn start_live_view(&self, camera_display_on: bool) -> Result<()> {
        self.exchange(|link| {
            link.session.start_live_view()?; // its reports carry the Picture Style
            // A restart after a hiccup keeps the session: start from x1 again, as the screen does.
            self.reset_view(link)?;
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

    /// Back to x1 with depth of field preview off, the user's Picture Style
    /// back, live view back to the camera's screen, then the session ends, so
    /// stop, exit and hiding the app never leave a session open.
    pub fn stop_live_view(&self) -> Result<()> {
        let Some(mut link) = lock(&self.link).take() else { return Ok(()) };
        let view = self.reset_view(&mut link);
        let style = link.user_style.map_or(Ok(()), |style| link.session.set_property(PICTURE_STYLE, style));
        let output = link.session.stop_live_view();
        let closed = link.session.close();
        view.and(style).and(output).and(closed).map_err(camera_error)
    }

    /// Back to x1 with depth of field preview off, where they were changed.
    fn reset_view(&self, link: &mut Link) -> std::result::Result<(), ptpip::Error> {
        if std::mem::take(&mut lock(&self.latest).zoom) > 1 {
            link.session.zoom(1)?;
        }
        if std::mem::take(&mut link.depth_of_field_preview) {
            link.session.depth_of_field_preview(false)?;
        }
        Ok(())
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
        LiveDetails { level: latest.level, af_frames: latest.af_frames(), histogram: latest.histogram.as_ref().map(|histogram| histogram.luminance.to_vec()) }
    }

    /// The newest histogram's luminance, R, G and B, from what came with the frames.
    pub fn histogram_channels(&self) -> Vec<Vec<u32>> {
        lock(&self.latest).histogram_channels()
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

    /// Live view magnified 1 (the whole picture), 5 or 10 times. At x10 the
    /// camera still sends the x5 pixels: the screen shows their central half.
    pub fn zoom(&self, level: u32) -> Result<()> {
        if ![1, 5, 10].contains(&level) {
            return Err(CameraError::Refused(format!("no x{level} magnification")));
        }
        self.exchange(|link| link.session.zoom(level))?;
        lock(&self.latest).zoom = level;
        Ok(())
    }

    /// The lens stops down to the set aperture while `on`.
    pub fn depth_of_field_preview(&self, on: bool) -> Result<()> {
        self.exchange(|link| {
            link.session.depth_of_field_preview(on)?;
            link.depth_of_field_preview = on;
            Ok(())
        })
    }

    /// The battery's charge: the camera's percent where it gives one, else
    /// Canon's level from what the camera last reported, a step.
    pub fn battery(&self) -> Result<Option<Battery>> {
        // A refusal isn't an error here: the step stands in for it.
        let percent = self.exchange(|link| link.session.battery_level()).or_else(|error| match error {
            CameraError::Refused(_) => Ok(None),
            other => Err(other),
        })?;
        let link = lock(&self.link);
        let properties = link.as_ref().ok_or_else(not_connected)?.session.properties();
        Ok(battery_from(percent, properties.value(BATTERY)))
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
    /// The magnification asked for: 5 or 10; 0 or 1 for the whole picture.
    zoom: u32,
    /// Block 0x11: luminance, R, G and B, 256 levels each.
    histogram: Option<Box<ptpip::Histogram>>,
    /// What AF is doing and where it found focus. Not kept from frame to
    /// frame: the camera sends it with every frame, zeros while AF isn't running.
    af: Option<AfZones>,
    /// The camera's own AF frames, likewise sent with every frame while it has any.
    af_boxes: Vec<AfBox>,
}

impl Latest {
    /// Takes in a GetViewFinderData reply. What it lacks keeps its last value.
    fn take_in(&mut self, data: &[u8]) {
        self.level = ptpip::viewfinder_level(data).or(self.level);
        self.sensor = ptpip::viewfinder_sensor(data).or(self.sensor);
        self.af_corner = ptpip::viewfinder_af_corner(data).or(self.af_corner);
        self.zoom_area = ptpip::viewfinder_zoom_area(data).or(self.zoom_area);
        self.af = ptpip::viewfinder_af(data);
        self.af_boxes = ptpip::viewfinder_af_frames(data);
        if let Some(histogram) = ptpip::viewfinder_histogram(data) {
            self.histogram = Some(Box::new(histogram));
        }
    }

    /// Luminance, R, G and B; none before the first histogram.
    fn histogram_channels(&self) -> Vec<Vec<u32>> {
        self.histogram.as_ref().map_or_else(Vec::new, |histogram| {
            [histogram.luminance, histogram.red, histogram.green, histogram.blue].map(|channel| channel.to_vec()).into()
        })
    }

    /// What live view's picture covers on the sensor: all of it, the magnify
    /// area at x5, that area's central half at x10.
    fn picture(&self) -> Option<Area> {
        let whole = |[x, y, width, height]: [u32; 4]| Area {
            x: x as f64,
            y: y as f64,
            width: width as f64,
            height: height as f64,
        };
        match self.zoom {
            5 => Some(whole(self.zoom_area?)),
            10 => {
                let area = whole(self.zoom_area?);
                let (width, height) = (area.width / 2.0, area.height / 2.0);
                Some(Area { x: area.x + width / 2.0, y: area.y + height / 2.0, width, height })
            }
            _ => {
                let [width, height] = self.sensor?;
                Some(whole([0, 0, width, height]))
            }
        }
    }

    /// As on the camera: its own AF frames (while it follows a subject) and a
    /// frame around each group of AF zones in focus, green while One-Shot
    /// holds focus, blue while Servo tracks; else the AF point.
    fn af_frames(&self) -> Vec<AfFrame> {
        let state = AfState::from_status(self.af.as_ref().map(|af| u64::from(af.status)));
        let mut frames = self.box_frames();
        if let Some(af) = self.af.as_ref().filter(|af| af.status != 0) {
            frames.extend(self.zone_frames(af, state));
        }
        if frames.is_empty() { self.point_frame(state) } else { frames }
    }

    /// The camera's own AF frames on the picture, but for the whole AF area
    /// (not drawn, as over CCAPI).
    fn box_frames(&self) -> Vec<AfFrame> {
        let Some(picture) = self.picture() else { return Vec::new() };
        self.af_boxes
            .iter()
            .map(|af| (box_state(af.status), af.area))
            .filter(|&(state, _)| state != AfState::Area)
            .map(|(state, [x, y, width, height])| AfFrame {
                x: ((x as f64 - picture.x) / picture.width) as f32,
                y: ((y as f64 - picture.y) / picture.height) as f32,
                width: (width as f64 / picture.width) as f32,
                height: (height as f64 / picture.height) as f32,
                state,
            })
            .collect()
    }

    /// The groups of zones in focus, each as the frame around it, on the picture.
    fn zone_frames(&self, af: &AfZones, state: AfState) -> Vec<AfFrame> {
        let (Some(picture), Some([sensor_width, sensor_height])) = (self.picture(), self.sensor) else {
            return Vec::new();
        };
        let zone = [sensor_width as f64 / af.grid[0] as f64, sensor_height as f64 / af.grid[1] as f64];
        zone_groups(&af.in_focus)
            .into_iter()
            .map(|[column, row, columns, rows]| AfFrame {
                x: ((column as f64 * zone[0] - picture.x) / picture.width) as f32,
                y: ((row as f64 * zone[1] - picture.y) / picture.height) as f32,
                width: (columns as f64 * zone[0] / picture.width) as f32,
                height: (rows as f64 * zone[1] / picture.height) as f32,
                state,
            })
            .filter(|frame| frame.x < 1.0 && frame.y < 1.0 && frame.x + frame.width > 0.0 && frame.y + frame.height > 0.0)
            .collect()
    }

    /// The AF point, in the middle of its magnify area, as a frame on the picture.
    fn point_frame(&self, state: AfState) -> Vec<AfFrame> {
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
            state,
        }]
    }
}

/// A frame's state from block 0x8, where the R6 Mark III also gives 18 for
/// focus held and 34 for the whole AF area in One-Shot (2026-10-29).
fn box_state(status: u32) -> AfState {
    match status {
        18 => AfState::Focused,
        34 => AfState::Area,
        other => AfState::from_status(Some(u64::from(other))),
    }
}

/// Zones touching one another (sides or corners) as one group: column, row,
/// columns and rows of the box around each, in the order first found.
fn zone_groups(zones: &[[u32; 2]]) -> Vec<[u32; 4]> {
    let mut left: Vec<[u32; 2]> = zones.to_vec();
    let mut groups = Vec::new();
    while let Some(first) = left.pop() {
        let (mut group, mut next) = (vec![first], vec![first]);
        while let Some([column, row]) = next.pop() {
            let (near, far): (Vec<_>, Vec<_>) =
                left.into_iter().partition(|&[c, r]| c.abs_diff(column) <= 1 && r.abs_diff(row) <= 1);
            left = far;
            group.extend(&near);
            next.extend(near);
        }
        let low = |i: usize| group.iter().map(|zone| zone[i]).min().unwrap_or(0);
        let high = |i: usize| group.iter().map(|zone| zone[i]).max().unwrap_or(0);
        groups.push([low(0), low(1), high(0) - low(0) + 1, high(1) - low(1) + 1]);
    }
    groups.reverse();
    groups
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

/// The camera's percent where it gives one, else Canon's level as a step.
fn battery_from(percent: Option<u8>, level: Option<u32>) -> Option<Battery> {
    match percent {
        Some(percent) => Some(Battery { percent, exact: true }),
        None => level.and_then(canon_codes::battery_percent).map(|percent| Battery { percent, exact: false }),
    }
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
        Area, EVENTS_EVERY, Latest, battery_from, camera_error, events_due, firmware, sensor_point, setting_code, setting_state,
        style_to_restore,
    };
    use crate::camera::{AfFrame, AfState, Battery, CameraError, Level};
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
    fn frames_keep_the_newest_histogram() {
        let mut latest = Latest::default();
        assert!(latest.histogram_channels().is_empty());
        // Block 0x11: Y, R, G, B, 256 levels each; all four are kept.
        let words: Vec<u32> = (0..4u32).flat_map(|channel| (0..256).map(move |level| channel * 1000 + level)).collect();
        latest.take_in(&reply(&[(0x11, &words)]));
        let channels = latest.histogram_channels();
        assert_eq!(channels.len(), 4);
        assert_eq!((channels[0].len(), channels[0][0], channels[0][255]), (256, 0, 255));
        assert_eq!((channels[1][7], channels[3][255]), (1007, 3255));
        // A frame without it keeps the last one.
        latest.take_in(&reply(&[(0x0e, &[6960, 4640])]));
        assert_eq!(latest.histogram_channels(), channels);
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

    /// Block 0x1c: status, six words not understood, a 39 x 27 grid, then a
    /// bit per AF zone (row by row, lowest bit first) set where in focus.
    fn af_block(status: u32, zones: &[[u32; 2]]) -> Vec<u32> {
        let mut words = vec![status, 0xa5, 0x19, 0xaa, 0xaa, 0xaa, 0xaa, 39, 27];
        words.resize(44, 0);
        for [column, row] in zones {
            let bit = row * 39 + column;
            words[9 + bit as usize / 32] |= 1 << (bit % 32);
        }
        words
    }

    #[test]
    fn the_af_frames_are_the_zones_in_focus_as_on_the_camera() {
        let sensor = (0x0e, &[6960, 4640][..]);
        let point = (0x05, &[2784, 1856][..]);
        // One-Shot holding focus on a pole (a tall strip of zones) and on a
        // single zone in the corner: a green frame around each.
        let strip: Vec<[u32; 2]> = (8..20).flat_map(|row| [[10, row], [11, row]]).collect();
        let mut zones = strip.clone();
        zones.push([38, 26]);
        let mut latest = Latest::default();
        latest.take_in(&reply(&[sensor, point, (0x1c, &af_block(17, &zones))]));
        let zone = |column: f32, row: f32, columns: f32, rows: f32, state| AfFrame {
            x: column / 39.0,
            y: row / 27.0,
            width: columns / 39.0,
            height: rows / 27.0,
            state,
        };
        let close = |a: &[AfFrame], b: &[AfFrame]| {
            a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| {
                    a.state == b.state
                        && [a.x - b.x, a.y - b.y, a.width - b.width, a.height - b.height].iter().all(|d| d.abs() < 1e-6)
                })
        };
        let focused = [zone(10.0, 8.0, 2.0, 12.0, AfState::Focused), zone(38.0, 26.0, 1.0, 1.0, AfState::Focused)];
        assert!(close(&latest.af_frames(), &focused), "{:?}", latest.af_frames());
        // A diagonal edge joins into one frame; Servo draws it blue.
        latest.take_in(&reply(&[sensor, point, (0x1c, &af_block(20, &[[5, 5], [6, 6], [7, 7]]))]));
        assert!(close(&latest.af_frames(), &[zone(5.0, 5.0, 3.0, 3.0, AfState::Tracking)]), "{:?}", latest.af_frames());
        // AF stopped (all zeros), or the block missing: the AF point, waiting.
        let waiting = [AfFrame { x: 0.45, y: 0.45, width: 0.1, height: 0.1, state: AfState::Waiting }];
        latest.take_in(&reply(&[sensor, point, (0x1c, &af_block(0, &[]))]));
        assert!(close(&latest.af_frames(), &waiting), "{:?}", latest.af_frames());
        latest.take_in(&reply(&[sensor, point, (0x1c, &af_block(17, &zones))]));
        latest.take_in(&reply(&[sensor, point]));
        assert!(close(&latest.af_frames(), &waiting), "{:?}", latest.af_frames());
        // Holding focus with no zones given: the AF point, green.
        latest.take_in(&reply(&[sensor, point, (0x1c, &[17])]));
        assert_eq!(latest.af_frames()[0].state, AfState::Focused);
        // At x5 the picture is the magnify area: zones off it aren't drawn.
        latest.take_in(&reply(&[sensor, point, (0x0d, &[2784, 1856, 1392, 928]), (0x1c, &af_block(17, &[[19, 13], [0, 0]]))]));
        latest.zoom = 5;
        let frames = latest.af_frames();
        assert_eq!(frames.len(), 1, "{frames:?}");
        assert!((frames[0].width - 5.0 / 39.0).abs() < 1e-6, "{frames:?}"); // a 39th of the sensor, which is 5 pictures wide
    }

    #[test]
    fn the_camera_s_own_af_frame_is_drawn_while_it_tracks_a_subject() {
        // After a tap the R6 Mark III follows a subject with one frame (block
        // 0x8): 48 waiting, 17 or 18 once focus is held (2026-10-29).
        let sensor = (0x0e, &[6960, 4640][..]);
        let point = (0x05, &[2784, 1856][..]);
        let frame = |status: u32| [1, status, 1, 696, 464, 1392, 928];
        let drawn = |status: u32| {
            let mut latest = Latest::default();
            latest.take_in(&reply(&[sensor, point, (0x08, &frame(status)), (0x1c, &af_block(0, &[]))]));
            latest.af_frames()
        };
        let at = |state| vec![AfFrame { x: 0.1, y: 0.1, width: 0.2, height: 0.2, state }];
        assert_eq!(drawn(0x30), at(AfState::Waiting));
        assert_eq!(drawn(0x11), at(AfState::Focused));
        assert_eq!(drawn(0x12), at(AfState::Focused));
        // The whole AF area (34 One-Shot, 32 Servo) isn't drawn, as over CCAPI:
        // the AF point instead, or the zones in focus.
        let waiting = vec![AfFrame { x: 0.45, y: 0.45, width: 0.1, height: 0.1, state: AfState::Waiting }];
        assert_eq!(drawn(0x22), waiting);
        let mut latest = Latest::default();
        latest.take_in(&reply(&[sensor, point, (0x08, &frame(0x20)), (0x1c, &af_block(20, &[[0, 0]]))]));
        let frames = latest.af_frames();
        assert_eq!((frames.len(), frames[0].state), (1, AfState::Tracking), "{frames:?}");
    }

    #[test]
    fn zoomed_the_picture_is_the_magnify_area() {
        let mut latest = Latest::default();
        latest.take_in(&reply(&[(0x0e, &[6960, 4640]), (0x05, &[2784, 1856]), (0x0d, &[2784, 1856, 1392, 928])]));
        let area = |x, y, width, height| Some((x, y, width, height));
        let seen = |latest: &Latest| latest.picture().map(|p| (p.x, p.y, p.width, p.height));
        assert_eq!(seen(&latest), area(0.0, 0.0, 6960.0, 4640.0));
        // x5: the camera sends the magnify area, 1:1 sensor pixels.
        latest.zoom = 5;
        assert_eq!(seen(&latest), area(2784.0, 1856.0, 1392.0, 928.0));
        let frame = AfFrame { x: 0.25, y: 0.25, width: 0.5, height: 0.5, state: AfState::Waiting };
        assert_eq!(latest.af_frames(), vec![frame]);
        assert_eq!(sensor_point(latest.picture().unwrap(), 0.5, 0.5), (3480, 2320));
        // x10: the same pixels, shown cropped to their central half.
        latest.zoom = 10;
        assert_eq!(seen(&latest), area(3132.0, 2088.0, 696.0, 464.0));
        let frame = AfFrame { x: 0.0, y: 0.0, width: 1.0, height: 1.0, state: AfState::Waiting };
        assert_eq!(latest.af_frames(), vec![frame]);
        assert_eq!(sensor_point(latest.picture().unwrap(), 0.0, 0.0), (3132, 2088));
        // Zoomed but no magnify area yet: no picture to map onto.
        let mut latest = Latest { zoom: 5, ..Latest::default() };
        latest.take_in(&reply(&[(0x0e, &[6960, 4640])]));
        assert!(latest.picture().is_none());
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
    fn the_battery_is_exact_where_the_camera_gives_a_percent() {
        // 84% on the R6 Mark III's screen, Canon's level 2 ("full") alongside (2026-10-29).
        assert_eq!(battery_from(Some(84), Some(2)), Some(Battery { percent: 84, exact: true }));
        // No percent (as some batteries may give): Canon's level, a step.
        assert_eq!(battery_from(None, Some(2)), Some(Battery { percent: 100, exact: false }));
        assert_eq!(battery_from(None, Some(5)), Some(Battery { percent: 25, exact: false }));
        assert_eq!(battery_from(None, Some(3)), None);
        assert_eq!(battery_from(None, None), None);
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

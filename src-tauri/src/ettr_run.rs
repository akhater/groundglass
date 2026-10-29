//! Expose to the right against the camera: read, move, wait for live view,
//! read again, for a bounded number of rounds. The reasoning is ettr.rs.
//! It stops at once, leaving the settings as they are, when a dial is turned,
//! the shooting mode changes or live view stops.

use std::time::Duration;

use serde::Serialize;

use crate::camera::{Camera, Result, SettingState};
use crate::ccapi::ExposureSetting;
use crate::ettr::{Dial, Reading, Search, followed, reading, spread};

/// What a round needs from the camera.
pub trait Controls {
    fn setting(&self, setting: ExposureSetting) -> Result<SettingState>;
    fn set(&self, setting: ExposureSetting, value: &str) -> Result<()>;
    /// The newest histogram: luminance, and R, G and B where the camera sends them.
    fn histogram(&self) -> Result<Vec<Vec<u32>>>;
    /// Waits for live view to show the last change.
    fn settle(&self);
}

/// How long live view takes to show a change, histogram included.
const SETTLE: Duration = Duration::from_millis(700);

impl Controls for Camera {
    fn setting(&self, setting: ExposureSetting) -> Result<SettingState> {
        self.exposure_setting(setting)
    }

    fn set(&self, setting: ExposureSetting, value: &str) -> Result<()> {
        self.set_exposure_setting(setting, value)
    }

    fn histogram(&self) -> Result<Vec<Vec<u32>>> {
        self.histogram_channels()
    }

    fn settle(&self) {
        std::thread::sleep(SETTLE);
    }
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "detail")]
pub enum Outcome {
    /// As bright as it gets with nothing clipped.
    Done,
    /// The best these settings can do: they are at the end of their range.
    Limited(Vec<ExposureSetting>),
    /// A dial was turned or the shooting mode changed: the settings are left as they are.
    Interrupted,
    /// Live view didn't follow a change (exposure simulation off, or frames
    /// stalled): the settings are put back as they were.
    NoResponse,
    /// Nothing ETTR can move: every setting pinned, ISO on auto, a setting the
    /// shooting mode sets (not M), or no histogram yet.
    Unavailable,
}

/// Rounds at most: a few to find the edge, a few for a scene that moves.
const ROUNDS: usize = 8;

/// Moves the settings not `pinned` to the brightest exposure that clips nothing.
pub fn run(camera: &impl Controls, pinned: &[ExposureSetting]) -> Result<Outcome> {
    let mut search = Search::default();
    // What the settings should read after the last round: anything else means a hand on a dial.
    let mut expected: Option<Vec<String>> = None;
    // The settings before the first change, and the histogram and step of the last one.
    let mut start: Option<Vec<(ExposureSetting, String)>> = None;
    let mut last: Option<(Vec<Vec<u32>>, i32)> = None;
    for _ in 0..ROUNDS {
        let states = [ExposureSetting::Iso, ExposureSetting::Av, ExposureSetting::Tv]
            .map(|setting| camera.setting(setting).map(|state| (setting, state)));
        let states = states.into_iter().collect::<Result<Vec<_>>>()?;
        let values: Vec<String> = states.iter().map(|(_, state)| state.value.clone()).collect();
        if expected.is_some_and(|expected| expected != values) {
            return Ok(Outcome::Interrupted);
        }
        let mut dials = Vec::new();
        for (setting, state) in &states {
            if pinned.contains(setting) {
                // A pinned setting on auto (Auto ISO) would undo every change.
                if state.value == "auto" {
                    return Ok(Outcome::Unavailable);
                }
                continue;
            }
            let Some(dial) = Dial::new(*setting, &state.value, &state.choices) else {
                return Ok(Outcome::Unavailable);
            };
            dials.push(dial);
        }
        if dials.is_empty() {
            return Ok(Outcome::Unavailable);
        }
        let histogram = camera.histogram()?;
        if let Some((before, by)) = &last
            && !followed(before, &histogram, *by)
        {
            let now = |setting: &ExposureSetting| states.iter().find(|(s, _)| s == setting).map(|(_, state)| &state.value);
            for (setting, value) in start.iter().flatten().filter(|(setting, value)| now(setting) != Some(value)) {
                camera.set(*setting, value)?;
            }
            return Ok(Outcome::NoResponse);
        }
        let Some(reading) = reading(&histogram) else {
            return Ok(Outcome::Unavailable);
        };
        let at = dials.iter().map(|dial| dial.at).sum();
        let Some(target) = search.next(at, &reading) else {
            return Ok(Outcome::Done);
        };
        let step = spread(&dials, target - at);
        if step.changes.is_empty() {
            // Stuck a third from the edge is the edge; further from it, the settings are the limit.
            let near = reading == Reading::Headroom(0);
            return Ok(if step.stuck.is_empty() || near { Outcome::Done } else { Outcome::Limited(step.stuck) });
        }
        if start.is_none() {
            let free = states.iter().filter(|(setting, _)| !pinned.contains(setting));
            start = Some(free.map(|(setting, state)| (*setting, state.value.clone())).collect());
        }
        let mut next = values;
        for (setting, value) in &step.changes {
            camera.set(*setting, value)?;
            let index = states.iter().position(|(s, _)| s == setting).unwrap_or_default();
            next[index] = value.clone();
        }
        if search.known_safe(target) {
            // Back to what was read safe a round ago: nothing left to learn.
            return Ok(Outcome::Done);
        }
        expected = Some(next);
        last = Some((histogram, target - at));
        camera.settle();
    }
    Ok(Outcome::Done)
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;
    use crate::camera::CameraError;
    use ExposureSetting::{Av, Iso, Tv};

    const SHUTTER: &[&str] = &[
        "1/1000", "1/800", "1/640", "1/500", "1/400", "1/320", "1/250", "1/200", "1/160", "1/125", "1/100", "1/80",
        "1/60", "1/50", "1/40", "1/30",
    ];
    const ISO: &[&str] = &["auto", "100", "125", "160", "200", "250", "320", "400", "500", "640", "800"];
    const APERTURE: &[&str] = &["f2.8", "f3.2", "f3.5", "f4.0", "f4.5", "f5.0", "f5.6", "f6.3", "f7.1", "f8.0"];

    /// A camera in M looking at a scene whose highlights clip above `limit`
    /// (brightness in thirds, from ettr::thirds). `curve` stretches how far
    /// below white the highlights look, as a picture style's curve would.
    struct Scene {
        values: RefCell<[(ExposureSetting, String, Vec<String>); 3]>,
        limit: i32,
        curve: f64,
        sets: Cell<usize>,
        settles: Cell<usize>,
        /// Runs after each settle: the user turning a dial, live view stopping.
        meanwhile: Box<dyn Fn(&Scene)>,
        live: Cell<bool>,
        /// Live view stuck at this brightness: exposure simulation off, or frames stalled.
        frozen: Cell<Option<i32>>,
    }

    fn list(choices: &[&str]) -> Vec<String> {
        choices.iter().map(|choice| choice.to_string()).collect()
    }

    fn scene(iso: &str, av: &str, tv: &str, limit: i32) -> Scene {
        Scene {
            values: RefCell::new([
                (Iso, iso.into(), list(ISO)),
                (Av, av.into(), list(APERTURE)),
                (Tv, tv.into(), list(SHUTTER)),
            ]),
            limit,
            curve: 1.0,
            sets: Cell::new(0),
            settles: Cell::new(0),
            meanwhile: Box::new(|_| {}),
            live: Cell::new(true),
            frozen: Cell::new(None),
        }
    }

    impl Scene {
        fn value(&self, setting: ExposureSetting) -> String {
            self.values.borrow().iter().find(|(s, ..)| *s == setting).unwrap().1.clone()
        }

        fn turn(&self, setting: ExposureSetting, value: &str) {
            self.values.borrow_mut().iter_mut().find(|(s, ..)| *s == setting).unwrap().1 = value.into();
        }

        fn brightness(&self) -> i32 {
            [Iso, Av, Tv].iter().map(|&s| crate::ettr::thirds(s, &self.value(s)).unwrap()).sum()
        }
    }

    impl Controls for Scene {
        fn setting(&self, setting: ExposureSetting) -> Result<SettingState> {
            let values = self.values.borrow();
            let (_, value, choices) = values.iter().find(|(s, ..)| *s == setting).unwrap();
            Ok(SettingState { value: value.clone(), choices: choices.clone() })
        }

        fn set(&self, setting: ExposureSetting, value: &str) -> Result<()> {
            self.sets.set(self.sets.get() + 1);
            self.turn(setting, value);
            Ok(())
        }

        fn histogram(&self) -> Result<Vec<Vec<u32>>> {
            if !self.live.get() {
                return Err(CameraError::LiveViewOff);
            }
            let over = self.frozen.get().unwrap_or_else(|| self.brightness()) - self.limit;
            // Highlights `-over` thirds below white (at 249 on the edge, white past it), on a
            // 2.2 gamma stretched by `curve`; the rest of the picture 2.5 times darker.
            let highlights = 249.0 * 2f64.powf(over as f64 / 3.0 / 2.2 * self.curve);
            let level = |value: f64| if over > 0 && value >= 249.0 { 255 } else { (value as usize).min(255) };
            let mut levels = vec![0; 256];
            levels[level(highlights / 2.5)] += 90_000;
            levels[level(highlights)] += 10_000;
            Ok(vec![levels])
        }

        fn settle(&self) {
            self.settles.set(self.settles.get() + 1);
            (self.meanwhile)(self);
        }
    }

    #[test]
    fn it_lands_one_third_below_clipping() {
        // ISO 100 + f5.6 + 1/500: brightness 0 - 15 - 27 = -42; clipping above 1/60 (-18), i.e. -33.
        let camera = scene("100", "f5.6", "1/500", -33);
        assert_eq!(run(&camera, &[Iso, Av]), Ok(Outcome::Done));
        assert_eq!(camera.value(Tv), "1/60");
        // There, one third more to see it clip, and back without another wait.
        assert_eq!((camera.sets.get(), camera.settles.get()), (3, 2));
    }

    #[test]
    fn at_the_edge_and_the_end_of_the_range_is_done() {
        // 1/30 is the slowest, and exactly at the edge: nothing to flash.
        let camera = scene("100", "f5.6", "1/30", -30);
        assert_eq!(run(&camera, &[Iso, Av]), Ok(Outcome::Done));
        // Far from the edge at the slowest: the shutter is what stops it.
        let camera = scene("100", "f5.6", "1/30", -20);
        assert_eq!(run(&camera, &[Iso, Av]), Ok(Outcome::Limited(vec![Tv])));
    }

    #[test]
    fn it_lands_there_whatever_the_curve() {
        for curve in [0.5, 0.8, 1.3, 2.0] {
            let mut camera = scene("100", "f5.6", "1/500", -33);
            camera.curve = curve;
            assert_eq!(run(&camera, &[Iso, Av]), Ok(Outcome::Done), "curve {curve}");
            assert_eq!(camera.value(Tv), "1/60", "curve {curve}");
        }
    }

    #[test]
    fn from_clipped_it_comes_down() {
        // ISO 800 + f2.8 + 1/30: 9 - 9 - 15 = -15, clipping above -21: ISO 200 (3) at most.
        let camera = scene("800", "f2.8", "1/30", -21);
        assert_eq!(run(&camera, &[Av, Tv]), Ok(Outcome::Done));
        assert_eq!(camera.value(Iso), "200");
    }

    #[test]
    fn too_bright_for_the_range_is_limited() {
        let camera = scene("800", "f2.8", "1/30", -33);
        assert_eq!(run(&camera, &[Av, Tv]), Ok(Outcome::Limited(vec![Iso])));
        assert_eq!(camera.value(Iso), "100");
    }

    #[test]
    fn a_turned_dial_stops_it() {
        let mut camera = scene("100", "f5.6", "1/500", -33);
        camera.meanwhile = Box::new(|camera| camera.turn(Av, "f8.0"));
        assert_eq!(run(&camera, &[Iso, Av]), Ok(Outcome::Interrupted));
        assert_eq!(camera.sets.get(), 1);
        assert_eq!(camera.value(Av), "f8.0");
    }

    #[test]
    fn live_view_stopping_stops_it() {
        let mut camera = scene("100", "f5.6", "1/500", -33);
        camera.meanwhile = Box::new(|camera| camera.live.set(false));
        assert_eq!(run(&camera, &[Iso, Av]), Err(CameraError::LiveViewOff));
        assert_eq!(camera.sets.get(), 1);
    }

    #[test]
    fn live_view_not_following_puts_the_settings_back() {
        let camera = scene("100", "f5.6", "1/500", -33);
        camera.frozen.set(Some(camera.brightness()));
        assert_eq!(run(&camera, &[Iso, Av]), Ok(Outcome::NoResponse));
        assert_eq!(camera.value(Tv), "1/500");
        // Stuck partly clipped: it doesn't walk to the darkest settings either.
        let camera = scene("800", "f2.8", "1/30", -18);
        camera.frozen.set(Some(camera.brightness()));
        assert_eq!(run(&camera, &[Av, Tv]), Ok(Outcome::NoResponse));
        assert_eq!(camera.value(Iso), "800");
        assert_eq!(camera.sets.get(), 2);
    }

    #[test]
    fn nothing_to_move_is_unavailable() {
        let camera = scene("100", "f5.6", "1/500", -33);
        assert_eq!(run(&camera, &[Iso, Av, Tv]), Ok(Outcome::Unavailable));
        // Auto ISO would undo every change.
        let camera = scene("auto", "f5.6", "1/500", -33);
        assert_eq!(run(&camera, &[Iso, Av]), Ok(Outcome::Unavailable));
        // In Av mode the camera sets the shutter: no choices.
        let camera = scene("100", "f5.6", "1/500", -33);
        camera.values.borrow_mut()[2].2.clear();
        assert_eq!(run(&camera, &[Iso, Av]), Ok(Outcome::Unavailable));
        assert_eq!(camera.sets.get(), 0);
    }

    /// A scene whose light keeps changing: clipped, dark, clipped...
    struct Flicker(Scene, Cell<bool>);

    impl Controls for Flicker {
        fn setting(&self, setting: ExposureSetting) -> Result<SettingState> {
            self.0.setting(setting)
        }
        fn set(&self, setting: ExposureSetting, value: &str) -> Result<()> {
            self.0.set(setting, value)
        }
        fn histogram(&self) -> Result<Vec<Vec<u32>>> {
            self.1.set(!self.1.get());
            let mut levels = vec![0; 256];
            levels[if self.1.get() { 255 } else { 10 }] = 1000;
            Ok(vec![levels])
        }
        fn settle(&self) {}
    }

    #[test]
    fn a_flickering_scene_still_ends() {
        let camera = Flicker(scene("400", "f5.6", "1/250", -33), Cell::new(false));
        assert!(run(&camera, &[Iso, Av]).is_ok());
        assert!(camera.0.sets.get() <= ROUNDS);
    }
}

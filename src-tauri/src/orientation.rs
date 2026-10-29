//! Portrait shooting: live view's picture always comes the sensor's way up, so
//! the screen turns it upright from the camera's roll, in quarter turns
//! clockwise. What is placed on the picture turns with it: the AF frames, a
//! tapped point (back to the sensor), and the level (from the nearest upright).

use serde::Serialize;

use crate::camera::{AfFrame, Level, LiveDetails};

/// The live details as the screen shows them: turned `turns` quarter turns clockwise.
#[derive(Debug, PartialEq, Serialize)]
pub struct Upright {
    pub turns: u8,
    #[serde(flatten)]
    pub details: LiveDetails,
}

/// Past the middle between two quarter turns by this much before the picture
/// turns, so it doesn't flicker there.
const MARGIN: f32 = 10.0;
/// Pointing up or down steeper than this, the roll says nothing about which way
/// is up: the picture stays as it is.
const STEEP: f32 = 60.0;

/// `details` turned upright, given the turns the screen `shown` until now;
/// `locked` (rotation lock), turned as shown whatever the camera does.
pub fn upright(details: LiveDetails, shown: u8, locked: bool) -> Upright {
    let turns = if locked { shown % 4 } else { quarter_turns(details.level, shown) };
    Upright {
        turns,
        details: LiveDetails {
            level: details.level.map(|level| from_upright(level, turns)),
            af_frames: details.af_frames.into_iter().map(|f| frame(f, turns)).collect(),
            histogram: details.histogram,
        },
    }
}

/// Quarter turns clockwise that show the picture upright, 0..3. The camera's
/// roll counts anticlockwise, seen from behind (found on the R6 Mark III, 2026-10-29).
fn quarter_turns(level: Option<Level>, shown: u8) -> u8 {
    let shown = shown % 4;
    match level {
        Some(level) if level.pitch.abs() <= STEEP && from_upright(level, shown).roll.abs() > 45.0 + MARGIN => {
            (-level.roll / 90.0).round().rem_euclid(4.0) as u8
        }
        _ => shown,
    }
}

/// Degrees from the upright `turns` stands for.
fn from_upright(level: Level, turns: u8) -> Level {
    let roll = (level.roll + 90.0 * f32::from(turns % 4)).rem_euclid(360.0);
    Level { roll: if roll > 180.0 { roll - 360.0 } else { roll }, ..level }
}

/// An AF frame on the sensor, placed on the picture turned upright.
fn frame(frame: AfFrame, turns: u8) -> AfFrame {
    let (x1, y1) = turn(frame.x.into(), frame.y.into(), turns);
    let (x2, y2) = turn((frame.x + frame.width).into(), (frame.y + frame.height).into(), turns);
    let (width, height) = if turns % 2 == 1 { (frame.height, frame.width) } else { (frame.width, frame.height) };
    AfFrame { x: x1.min(x2) as f32, y: y1.min(y2) as f32, width, height, ..frame }
}

/// A point on the picture turned upright (fractions across), back on the sensor.
pub fn sensor_point(x: f64, y: f64, turns: u8) -> (f64, f64) {
    turn(x, y, 4 - turns % 4)
}

/// A point (fractions across) turned `turns` quarter turns clockwise with the picture.
fn turn(x: f64, y: f64, turns: u8) -> (f64, f64) {
    match turns % 4 {
        0 => (x, y),
        1 => (1.0 - y, x),
        2 => (1.0 - x, 1.0 - y),
        _ => (y, 1.0 - x),
    }
}

#[cfg(test)]
mod tests {
    use super::{frame, from_upright, quarter_turns, sensor_point, upright};
    use crate::camera::{AfFrame, AfState, Level, LiveDetails};

    fn level(roll: f32) -> Option<Level> {
        Some(Level { roll, pitch: 0.0 })
    }

    #[test]
    fn the_roll_picks_the_nearest_quarter_turn() {
        // The camera's roll counts anticlockwise, seen from behind: -90 is turned
        // clockwise, so the picture turns a quarter clockwise to stand upright.
        assert_eq!(quarter_turns(level(-3.0), 0), 0);
        assert_eq!(quarter_turns(level(-88.0), 0), 1);
        assert_eq!(quarter_turns(level(91.0), 0), 3);
        assert_eq!(quarter_turns(level(-179.0), 0), 2);
        assert_eq!(quarter_turns(level(178.0), 0), 2);
        assert_eq!(quarter_turns(level(2.0), 3), 0);
    }

    #[test]
    fn the_turn_holds_until_the_roll_is_well_past_the_middle() {
        assert_eq!(quarter_turns(level(-50.0), 0), 0);
        assert_eq!(quarter_turns(level(-56.0), 0), 1);
        assert_eq!(quarter_turns(level(-40.0), 1), 1);
        assert_eq!(quarter_turns(level(-34.0), 1), 0);
        assert_eq!(quarter_turns(level(50.0), 0), 0);
        assert_eq!(quarter_turns(level(130.0), 3), 3);
        assert_eq!(quarter_turns(level(150.0), 3), 2);
    }

    #[test]
    fn without_a_level_or_pointing_up_or_down_the_turn_holds() {
        // Straight up or down, the roll says nothing about which way is up.
        assert_eq!(quarter_turns(None, 1), 1);
        assert_eq!(quarter_turns(Some(Level { roll: -90.0, pitch: -75.0 }), 0), 0);
        assert_eq!(quarter_turns(Some(Level { roll: 0.0, pitch: 70.0 }), 3), 3);
        assert_eq!(quarter_turns(Some(Level { roll: -90.0, pitch: 30.0 }), 0), 1);
    }

    #[test]
    fn the_level_reads_from_the_nearest_upright() {
        let turned = |roll, turns| from_upright(Level { roll, pitch: -4.0 }, turns);
        assert_eq!(turned(1.5, 0), Level { roll: 1.5, pitch: -4.0 });
        assert_eq!(turned(-88.5, 1), Level { roll: 1.5, pitch: -4.0 });
        assert_eq!(turned(88.0, 3), Level { roll: -2.0, pitch: -4.0 });
        assert_eq!(turned(-179.0, 2), Level { roll: 1.0, pitch: -4.0 });
        assert_eq!(turned(178.0, 2), Level { roll: -2.0, pitch: -4.0 });
    }

    #[test]
    fn af_frames_turn_with_the_picture() {
        let f = AfFrame { x: 0.1, y: 0.2, width: 0.3, height: 0.1, state: AfState::Focused };
        let place = |turns| {
            let g = frame(f, turns);
            assert_eq!(g.state, AfState::Focused);
            [g.x, g.y, g.width, g.height].map(|v| (v * 1e5).round() / 1e5)
        };
        // Near the sensor's top-left corner, 0.3 wide and 0.1 high.
        assert_eq!(place(0), [0.1, 0.2, 0.3, 0.1]);
        // A quarter turn clockwise: the left edge becomes the top, near the right.
        assert_eq!(place(1), [0.7, 0.1, 0.1, 0.3]);
        assert_eq!(place(2), [0.6, 0.7, 0.3, 0.1]);
        assert_eq!(place(3), [0.2, 0.6, 0.1, 0.3]);
    }

    #[test]
    fn a_tap_goes_back_to_the_sensor() {
        // The picture's top-right corner, a quarter turn clockwise, is the sensor's top-left.
        assert_eq!(sensor_point(1.0, 0.0, 1), (0.0, 0.0));
        assert_eq!(sensor_point(1.0, 0.0, 3), (1.0, 1.0));
        assert_eq!(sensor_point(0.25, 0.5, 0), (0.25, 0.5));
        assert_eq!(sensor_point(0.25, 0.5, 2), (0.75, 0.5));
        // A tap on an AF frame's corner lands on that frame's corner on the sensor.
        for turns in 0..4 {
            let f = frame(AfFrame { x: 0.25, y: 0.5, width: 0.0, height: 0.0, state: AfState::Waiting }, turns);
            assert_eq!(sensor_point(f.x.into(), f.y.into(), turns), (0.25, 0.5));
        }
    }

    #[test]
    fn the_details_come_turned() {
        let details = LiveDetails {
            level: Some(Level { roll: -89.0, pitch: 2.0 }),
            af_frames: vec![AfFrame { x: 0.0, y: 0.0, width: 0.5, height: 0.25, state: AfState::Waiting }],
            histogram: Some(vec![1, 2]),
        };
        let shown = upright(details, 0, false);
        assert_eq!(shown.turns, 1);
        assert_eq!(shown.details.level, Some(Level { roll: 1.0, pitch: 2.0 }));
        assert_eq!(shown.details.af_frames[0], AfFrame { x: 0.75, y: 0.0, width: 0.25, height: 0.5, state: AfState::Waiting });
        assert_eq!(shown.details.histogram, Some(vec![1, 2]));
    }

    #[test]
    fn locked_the_picture_keeps_its_turn() {
        let details = |roll| LiveDetails { level: Some(Level { roll, pitch: 0.0 }), af_frames: vec![], histogram: None };
        // Held upright at a quarter turn, the camera now level: the level reads from the held turn.
        let held = upright(details(0.0), 1, true);
        assert_eq!(held.turns, 1);
        assert_eq!(held.details.level, Some(Level { roll: 90.0, pitch: 0.0 }));
        assert_eq!(upright(details(-90.0), 0, true).turns, 0);
    }
}

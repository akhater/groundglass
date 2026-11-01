//! Expose to the right, the reasoning only: how bright a histogram is, where to
//! go next, and which settings make up the difference. No camera here; the
//! rounds against the camera are ettr_run.rs.
//!
//! Brightness is counted in thirds of a stop, higher is brighter, so the three
//! settings add up: a slower shutter, a wider aperture or a higher ISO each add.

use crate::ccapi::ExposureSetting;

/// A setting's label as the camera writes it ("1/125", "0\"8", "f5.6", "6400")
/// in thirds of a stop of brightness. None for "auto", "bulb" and the unknown.
pub fn thirds(setting: ExposureSetting, label: &str) -> Option<i32> {
    let stops = match setting {
        ExposureSetting::Iso => (label.parse::<f64>().ok()? / 100.0).log2(),
        ExposureSetting::Av => -2.0 * label.strip_prefix('f')?.parse::<f64>().ok()?.log2(),
        ExposureSetting::Tv => seconds(label)?.log2(),
    };
    // Labels are rounded ("1/125" is 2^-7 s), so the nearest third is the true one.
    stops.is_finite().then(|| (stops * 3.0).round() as i32)
}

/// "1/125", "0\"8" (0.8 s), "2\"" in seconds.
fn seconds(label: &str) -> Option<f64> {
    if let Some(denominator) = label.strip_prefix("1/") {
        return Some(1.0 / denominator.parse::<f64>().ok()?);
    }
    let (whole, tenths) = label.split_once('"')?;
    let tenths = if tenths.is_empty() { 0.0 } else { tenths.parse::<f64>().ok()? / 10.0 };
    Some(whole.parse::<f64>().ok()? + tenths)
}

/// The most one round brightens: 3 stops, so a black picture is no leap in the dark.
pub const MAX_STEP_UP: i32 = 9;

/// What a histogram says about the exposure.
#[derive(Debug, PartialEq)]
pub enum Reading {
    /// More than a tiny spot is white: the fraction of the worst channel's samples.
    Clipped(f64),
    /// Thirds of a stop that can be added before anything but a tiny spot clips.
    Headroom(i32),
}

/// From one or more channels of 256 levels (luminance, or R, G and B too).
/// None when no channel holds any samples.
pub fn reading(channels: &[Vec<u32>]) -> Option<Reading> {
    let mut clipped: Option<f64> = None;
    let mut brightest = 0;
    for levels in channels {
        let total: u64 = levels.iter().map(|&count| u64::from(count)).sum();
        if total == 0 {
            continue;
        }
        let white: u64 = levels.get(CLIP_LEVEL..).unwrap_or(&[]).iter().map(|&count| u64::from(count)).sum();
        clipped = Some(clipped.unwrap_or(0.0).max(white as f64 / total as f64));
        // The brightest level with more than a tiny spot at or above it.
        let mut above = 0;
        let top = levels.iter().enumerate().rev().find_map(|(level, &count)| {
            above += u64::from(count);
            (above as f64 > TINY * total as f64).then_some(level)
        });
        brightest = brightest.max(top.unwrap_or(0));
    }
    let clipped = clipped?;
    if clipped > TINY {
        return Some(Reading::Clipped(clipped));
    }
    if brightest == 0 {
        return Some(Reading::Headroom(MAX_STEP_UP));
    }
    // The preview is gamma encoded: level ~ light^(1/2.2).
    let stops = GAMMA * (CLIP_LEVEL as f64 / brightest as f64).log2();
    Some(Reading::Headroom(((stops * 3.0).floor() as i32).clamp(0, MAX_STEP_UP)))
}

/// From this level up counts as white: the preview rolls off just before 255.
const CLIP_LEVEL: usize = 250;
/// A spot this small may clip (1 sample in 1000): a lamp, a glint of sun.
const TINY: f64 = 0.001;
/// The preview's encoding, close enough to Picture Style Neutral's curve.
const GAMMA: f64 = 2.2;

/// The search for the brightest exposure that clips nothing, from one reading
/// to the next. It remembers the brightest exposure seen safe and the darkest
/// seen clipped, and never goes back to a clipped one, so it closes in.
#[derive(Default)]
pub struct Search {
    safe: Option<i32>,
    clipped: Option<i32>,
}

impl Search {
    /// The brightness to try after `reading` at `at`, or None when `at` is the one.
    pub fn next(&mut self, at: i32, reading: &Reading) -> Option<i32> {
        match *reading {
            Reading::Clipped(white) => {
                self.clipped = Some(at);
                // A safe reading at or above this one is out of date (the light changed).
                self.safe = self.safe.filter(|&safe| safe < at);
                Some(match self.safe {
                    Some(safe) => safe + (at - safe) / 2,
                    None if white > MOSTLY_WHITE => at - 6,
                    None => at - 3,
                })
            }
            Reading::Headroom(headroom) => {
                self.safe = Some(at);
                self.clipped = self.clipped.filter(|&clipped| clipped > at);
                // Under a third to go, by the curve: one third more shows where the edge really is.
                let up = at + headroom.max(1);
                let target = self.clipped.map_or(up, |clipped| up.min(clipped - 1));
                (target > at).then_some(target)
            }
        }
    }
}

impl Search {
    /// Whether `at` was read clipping nothing, and nothing brighter since clipped below it.
    pub fn known_safe(&self, at: i32) -> bool {
        self.safe == Some(at)
    }
}

/// More white than this and the first step down is 2 stops instead of 1.
const MOSTLY_WHITE: f64 = 0.1;

/// Whether live view's picture moved the way a step of `by` thirds should move it,
/// judged on the mean level of the first channel (luminance) `before` and `after`.
pub fn followed(before: &[Vec<u32>], after: &[Vec<u32>], by: i32) -> bool {
    let (Some(before), Some(after)) = (mean(before), mean(after)) else { return false };
    if by < 0 && before >= CLIP_LEVEL as f64 {
        return true;
    }
    // A third of a stop moves the mean about a tenth; noise moves it far less.
    (after - before) * f64::from(by.signum()) > 0.03 * before.max(10.0)
}

fn mean(channels: &[Vec<u32>]) -> Option<f64> {
    let levels = channels.first()?;
    let total: f64 = levels.iter().map(|&count| f64::from(count)).sum();
    let sum: f64 = levels.iter().enumerate().map(|(level, &count)| level as f64 * f64::from(count)).sum();
    (total > 0.0).then(|| sum / total)
}

/// A setting ETTR may move: where it is and where the camera lets it go, in thirds.
pub struct Dial {
    pub setting: ExposureSetting,
    pub at: i32,
    /// Where the run began with it, and `spread` works from: a run keeps the first
    /// round's `start`s, so the same exposure gets the same settings in every round.
    pub start: i32,
    choices: Vec<(i32, String)>,
}

/// ISO 100 in thirds, the lowest ETTR uses: 50 (L) is an expanded value, not the base ISO.
const BASE_ISO: i32 = 0;

impl Dial {
    /// None when the value has no brightness (auto) or the camera offers no choice
    /// (the shooting mode sets it).
    pub fn new(setting: ExposureSetting, value: &str, choices: &[String]) -> Option<Self> {
        let at = thirds(setting, value)?;
        let lowest = if setting == ExposureSetting::Iso { BASE_ISO } else { i32::MIN };
        let choices: Vec<_> = choices
            .iter()
            .filter_map(|choice| Some((thirds(setting, choice)?, choice.clone())))
            .filter(|(thirds, _)| *thirds >= lowest)
            .collect();
        (!choices.is_empty()).then_some(Self { setting, at, start: at.max(lowest), choices })
    }

    /// The choice from the start toward `want` that doesn't pass it, or the end of the
    /// range when darker is asked past it. None when that is no move toward `want`.
    fn toward(&self, want: i32) -> Option<i32> {
        let thirds = self.choices.iter().map(|(thirds, _)| *thirds);
        let choice = thirds.clone().filter(|thirds| *thirds <= want).max().or_else(|| thirds.min())?;
        ((choice - self.start).signum() == (want - self.start).signum() && choice != self.start).then_some(choice)
    }

    /// Whether it started above its lowest choice.
    fn above_lowest(&self) -> bool {
        self.choices.iter().any(|(thirds, _)| *thirds < self.start)
    }

    /// The choice at `at`, as the camera writes it.
    fn label(&self, at: i32) -> Option<String> {
        self.choices.iter().find(|(thirds, _)| *thirds == at).map(|(_, label)| label.clone())
    }

    /// Already as far as it goes in the direction of `by`.
    fn at_end(&self, at: i32, by: i32) -> bool {
        let thirds = self.choices.iter().map(|(thirds, _)| *thirds);
        if by > 0 { thirds.max() <= Some(at) } else { thirds.min() >= Some(at) }
    }
}

/// The settings to send, and when they can't give all that was asked, the ones at the end of their range.
#[derive(Debug, Default, PartialEq)]
pub struct Move {
    pub changes: Vec<(ExposureSetting, String)>,
    pub stuck: Vec<ExposureSetting>,
}

/// The settings for `by` thirds from where `dials` are now, worked out from where
/// they started, so a step back undoes the step that overshot.
///
/// Brighter: shutter, then aperture, ISO last (it adds noise). With ISO free and
/// above its lowest, the aperture comes first, so the shutter slows only once
/// nothing but raising ISO is left. Darker: ISO first, then shutter, then aperture.
/// A step the camera's choices can't match lands on the darker side. Then `trade`.
/// What darkens is sent first: a step cut short is never brighter than asked.
pub fn spread(dials: &[Dial], by: i32) -> Move {
    use ExposureSetting::{Av, Iso, Tv};
    let iso_to_lower = dials.iter().any(|dial| dial.setting == Iso && dial.above_lowest());
    let by = by + dials.iter().map(|dial| dial.at - dial.start).sum::<i32>();
    let order = match (by > 0, iso_to_lower) {
        (true, true) => [Av, Tv, Iso],
        (true, false) => [Tv, Av, Iso],
        (false, _) => [Iso, Tv, Av],
    };
    let dials: Vec<&Dial> = order.iter().filter_map(|&setting| dials.iter().find(|dial| dial.setting == setting)).collect();
    let mut left = by;
    let mut landing = Vec::new();
    let mut ends = Vec::new();
    for dial in &dials {
        let mut at = dial.start;
        if left.signum() == by.signum()
            && let Some(thirds) = dial.toward(dial.start + left)
        {
            left -= thirds - dial.start;
            at = thirds;
        }
        landing.push(at);
        ends.push((dial.setting, dial.at_end(at, by)));
    }
    trade(&dials, &mut landing);
    let mut moves: Vec<_> = dials.iter().zip(landing).filter(|(dial, at)| *at != dial.at).collect();
    moves.sort_by_key(|(dial, at)| (at - dial.at).signum());
    let changes = moves.into_iter().filter_map(|(dial, at)| Some((dial.setting, dial.label(at)?))).collect();
    let mut next = Move { changes, stuck: vec![] };
    if left != 0 && left.signum() == by.signum() {
        next.stuck = ends.into_iter().filter_map(|(setting, end)| end.then_some(setting)).collect();
    }
    next
}

/// Lowers ISO as far as the aperture can make up to the third, for the same exposure.
/// The shutter takes no part: it never goes slower just to lower ISO.
fn trade(dials: &[&Dial], landing: &mut [i32]) {
    let index = |setting| dials.iter().position(|dial| dial.setting == setting);
    let (Some(iso), Some(av)) = (index(ExposureSetting::Iso), index(ExposureSetting::Av)) else { return };
    let (at_iso, at_av) = (landing[iso], landing[av]);
    let makes_up = |thirds: i32| dials[av].choices.iter().any(|(choice, _)| *choice == at_av + at_iso - thirds);
    let choices = dials[iso].choices.iter().map(|(thirds, _)| *thirds);
    if let Some(lowest) = choices.filter(|&thirds| thirds < at_iso && makes_up(thirds)).min() {
        landing[iso] = lowest;
        landing[av] = at_av + at_iso - lowest;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ExposureSetting::{Av, Iso, Tv};

    fn dial(setting: ExposureSetting, value: &str, choices: &[&str]) -> Dial {
        let choices: Vec<String> = choices.iter().map(|choice| choice.to_string()).collect();
        Dial::new(setting, value, &choices).unwrap()
    }

    const SHUTTER: &[&str] = &["bulb", "1/250", "1/200", "1/160", "1/125", "1/100", "1/80", "1/60", "1/50"];
    const ISO: &[&str] = &["auto", "100", "125", "160", "200", "250", "320", "400"];
    const APERTURE: &[&str] = &["f2.8", "f3.2", "f3.5", "f4.0", "f4.5", "f5.0", "f5.6"];

    const ISO_WITH_L: &[&str] = &["auto", "50", "100", "125", "160", "200", "400"];

    fn changes(changes: &[(ExposureSetting, &str)]) -> Vec<(ExposureSetting, String)> {
        changes.iter().map(|&(setting, value)| (setting, value.to_string())).collect()
    }

    /// `dial` as a later round sees it: the run began with it on `value`.
    fn began(mut dial: Dial, value: &str) -> Dial {
        dial.start = thirds(dial.setting, value).unwrap();
        dial
    }

    #[test]
    fn iso_stops_at_100_with_50_on_offer() {
        let dials = [dial(Iso, "160", ISO_WITH_L)];
        assert_eq!(spread(&dials, -2), Move { changes: changes(&[(Iso, "100")]), stuck: vec![] });
        assert_eq!(spread(&dials, -5), Move { changes: changes(&[(Iso, "100")]), stuck: vec![Iso] });
    }

    #[test]
    fn from_50_iso_comes_up_to_100_and_the_rest_makes_up_for_it() {
        let dials = [dial(Iso, "50", ISO_WITH_L), dial(Tv, "1/125", SHUTTER)];
        // The same exposure: a stop less from the shutter, sent first.
        assert_eq!(spread(&dials, 0), Move { changes: changes(&[(Tv, "1/250"), (Iso, "100")]), stuck: vec![] });
        // A stop brighter is ISO's own stop.
        assert_eq!(spread(&dials, 3), Move { changes: changes(&[(Iso, "100")]), stuck: vec![] });
        assert_eq!(spread(&dials, 5), Move { changes: changes(&[(Tv, "1/80"), (Iso, "100")]), stuck: vec![] });
    }

    #[test]
    fn iso_comes_down_as_far_as_the_aperture_can_make_up() {
        let shutter = || dial(Tv, "1/125", SHUTTER);
        let dials = [dial(Iso, "400", ISO), dial(Av, "f5.6", APERTURE), shutter()];
        assert_eq!(spread(&dials, 0), Move { changes: changes(&[(Iso, "100"), (Av, "f2.8")]), stuck: vec![] });
        // Darker too: ISO gives the third and what the aperture buys.
        assert_eq!(spread(&dials, -2), Move { changes: changes(&[(Iso, "100"), (Av, "f3.5")]), stuck: vec![] });
        // Only two thirds of aperture left.
        let dials = [dial(Iso, "400", ISO), dial(Av, "f3.5", APERTURE), shutter()];
        assert_eq!(spread(&dials, 0), Move { changes: changes(&[(Iso, "250"), (Av, "f2.8")]), stuck: vec![] });
    }

    #[test]
    fn the_shutter_never_slows_to_lower_iso() {
        let dials = [dial(Iso, "400", ISO), dial(Tv, "1/125", SHUTTER)];
        assert_eq!(spread(&dials, 0), Move::default());
    }

    #[test]
    fn a_trade_keeps_the_exposure_exact() {
        // ISO in whole stops, and no aperture that makes up a whole number of them.
        let dials = [dial(Iso, "400", &["100", "200", "400"]), dial(Av, "f3.2", &["f2.0", "f2.8", "f3.2"])];
        assert_eq!(spread(&dials, 0), Move::default());
    }

    #[test]
    fn above_100_the_aperture_brightens_before_the_shutter() {
        let dials = [dial(Iso, "200", ISO), dial(Av, "f4.0", APERTURE), dial(Tv, "1/125", SHUTTER)];
        assert_eq!(spread(&dials, 3), Move { changes: changes(&[(Av, "f2.8")]), stuck: vec![] });
        // Past the widest the shutter is the only way left, and ISO stays where it was.
        assert_eq!(spread(&dials, 5), Move { changes: changes(&[(Av, "f2.8"), (Tv, "1/80")]), stuck: vec![] });
        // Aperture left over goes to ISO.
        assert_eq!(spread(&dials, 1), Move { changes: changes(&[(Iso, "125"), (Av, "f2.8")]), stuck: vec![] });
    }

    #[test]
    fn with_no_iso_to_lower_the_shutter_brightens_first() {
        // At 100.
        let dials = [dial(Iso, "100", ISO), dial(Av, "f4.0", APERTURE), dial(Tv, "1/125", SHUTTER)];
        assert_eq!(spread(&dials, 2), Move { changes: changes(&[(Tv, "1/80")]), stuck: vec![] });
        // ISO pinned.
        let dials = [dial(Av, "f4.0", APERTURE), dial(Tv, "1/125", SHUTTER)];
        assert_eq!(spread(&dials, 2), Move { changes: changes(&[(Tv, "1/80")]), stuck: vec![] });
    }

    #[test]
    fn every_round_spreads_from_where_the_run_began() {
        // A third past the edge on the shutter: back on the shutter, not on ISO.
        let dials = [
            dial(Iso, "200", ISO),
            began(dial(Av, "f2.8", APERTURE), "f4.0"),
            began(dial(Tv, "1/100", SHUTTER), "1/125"),
        ];
        assert_eq!(spread(&dials, -1), Move { changes: changes(&[(Tv, "1/125")]), stuck: vec![] });
        // Traded down to 100, then a stop brighter: only ISO moves, back up.
        let dials = [
            began(dial(Iso, "100", ISO), "400"),
            began(dial(Av, "f2.8", APERTURE), "f5.6"),
            dial(Tv, "1/125", SHUTTER),
        ];
        assert_eq!(spread(&dials, 3), Move { changes: changes(&[(Iso, "200")]), stuck: vec![] });
    }

    #[test]
    fn a_dial_needs_a_brightness_and_choices() {
        assert!(Dial::new(Iso, "auto", &["100".into()]).is_none());
        assert!(Dial::new(Av, "f4.0", &[]).is_none());
        assert!(Dial::new(Tv, "1/125", &["bulb".into()]).is_none());
    }

    #[test]
    fn brighter_moves_the_shutter_before_iso() {
        let dials = [dial(Iso, "100", ISO), dial(Tv, "1/125", SHUTTER)];
        assert_eq!(spread(&dials, 2), Move { changes: changes(&[(Tv, "1/80")]), stuck: vec![] });
        // Past the shutter's slowest, ISO makes up the rest.
        assert_eq!(spread(&dials, 6), Move { changes: changes(&[(Tv, "1/50"), (Iso, "160")]), stuck: vec![] });
    }

    #[test]
    fn darker_lowers_iso_first() {
        let dials = [dial(Tv, "1/125", SHUTTER), dial(Iso, "200", ISO)];
        assert_eq!(spread(&dials, -2), Move { changes: changes(&[(Iso, "125")]), stuck: vec![] });
        assert_eq!(spread(&dials, -5), Move { changes: changes(&[(Iso, "100"), (Tv, "1/200")]), stuck: vec![] });
    }

    #[test]
    fn the_aperture_moves_after_the_shutter() {
        // The shutter is at its slowest.
        let dials = [dial(Av, "f4.0", APERTURE), dial(Tv, "1/50", SHUTTER)];
        assert_eq!(spread(&dials, 2), Move { changes: changes(&[(Av, "f3.2")]), stuck: vec![] });
    }

    #[test]
    fn settings_at_their_end_are_stuck() {
        let dials = [dial(Iso, "100", ISO), dial(Tv, "1/250", SHUTTER)];
        assert_eq!(spread(&dials, -3), Move { changes: vec![], stuck: vec![Iso, Tv] });
        // Some of the way, then stuck.
        let dials = [dial(Iso, "320", ISO), dial(Tv, "1/50", SHUTTER)];
        assert_eq!(spread(&dials, 3), Move { changes: changes(&[(Iso, "400")]), stuck: vec![Tv, Iso] });
    }

    #[test]
    fn half_stop_choices_never_overshoot_brighter() {
        let dials = [dial(Tv, "1/125", &["1/180", "1/125", "1/90", "1/60"])];
        assert_eq!(spread(&dials, 1), Move::default());
        assert_eq!(spread(&dials, 2), Move { changes: changes(&[(Tv, "1/90")]), stuck: vec![] });
    }

    #[test]
    fn a_step_shows_in_the_picture() {
        let mid = vec![channel(&[(100, 1000)])];
        let brighter = vec![channel(&[(112, 1000)])];
        assert!(followed(&mid, &brighter, 1));
        assert!(followed(&brighter, &mid, -1));
        // The wrong way, or not at all: live view isn't following the settings.
        assert!(!followed(&mid, &brighter, -1));
        assert!(!followed(&mid, &mid, 3));
        assert!(!followed(&mid, &[channel(&[(101, 1000)])], 1));
        // Near black a small move still counts.
        assert!(followed(&[channel(&[(2, 1000)])], &[channel(&[(3, 1000)])], 1));
        assert!(!followed(&[], &mid, 1));
        // All white shows nothing of a step down until it comes back from white.
        let white = vec![channel(&[(255, 1000)])];
        assert!(followed(&white, &white, -6));
        assert!(!followed(&white, &white, 1));
    }

    #[test]
    fn headroom_is_taken_in_one_step() {
        assert_eq!(Search::default().next(0, &Reading::Headroom(6)), Some(6));
        // Too little to measure: one third more is tried, to find the edge rather than guess it.
        assert_eq!(Search::default().next(-4, &Reading::Headroom(0)), Some(-3));
    }

    #[test]
    fn clipping_steps_down_further_the_more_is_white() {
        assert_eq!(Search::default().next(0, &Reading::Clipped(0.02)), Some(-3));
        assert_eq!(Search::default().next(0, &Reading::Clipped(0.3)), Some(-6));
    }

    #[test]
    fn it_closes_in_between_safe_and_clipped() {
        let mut search = Search::default();
        assert_eq!(search.next(0, &Reading::Headroom(6)), Some(6));
        // Brighter than the preview's curve let on: halfway back.
        assert_eq!(search.next(6, &Reading::Clipped(0.2)), Some(3));
        // Never back up to what clipped: one third short of it.
        assert_eq!(search.next(3, &Reading::Headroom(4)), Some(5));
        assert_eq!(search.next(5, &Reading::Headroom(2)), None);
    }

    #[test]
    fn one_third_below_clipping_is_the_answer() {
        let mut search = Search::default();
        assert_eq!(search.next(4, &Reading::Headroom(1)), Some(5));
        assert_eq!(search.next(5, &Reading::Clipped(0.01)), Some(4));
        assert_eq!(search.next(4, &Reading::Headroom(1)), None);
        assert!(search.known_safe(4));
        assert!(!search.known_safe(5));
    }

    /// 256 levels, `samples` at each (level, count).
    fn channel(samples: &[(usize, u32)]) -> Vec<u32> {
        let mut levels = vec![0; 256];
        for &(level, count) in samples {
            levels[level] += count;
        }
        levels
    }

    #[test]
    fn a_white_sky_is_clipped() {
        assert_eq!(reading(&[channel(&[(128, 95_000), (255, 5_000)])]), Some(Reading::Clipped(0.05)));
        // Just short of 255 is white too: the preview rolls off before the top level.
        assert!(matches!(reading(&[channel(&[(128, 95_000), (251, 5_000)])]), Some(Reading::Clipped(_))));
    }

    #[test]
    fn a_tiny_bright_spot_may_clip() {
        // A lamp: 5 samples in 10 000 white, the rest well below.
        assert_eq!(reading(&[channel(&[(200, 99_950), (255, 50)])]), Some(Reading::Headroom(2)));
    }

    #[test]
    fn headroom_is_measured_from_the_brightest_samples() {
        assert_eq!(reading(&[channel(&[(30, 50_000), (128, 50_000)])]), Some(Reading::Headroom(6)));
        assert_eq!(reading(&[channel(&[(245, 100_000)])]), Some(Reading::Headroom(0)));
        // A black picture: a big step, but a bounded one.
        assert_eq!(reading(&[channel(&[(0, 100_000)])]), Some(Reading::Headroom(MAX_STEP_UP)));
    }

    #[test]
    fn any_clipped_colour_counts() {
        let luminance = channel(&[(180, 100_000)]);
        let red = channel(&[(180, 98_000), (255, 2_000)]);
        let green = channel(&[(100, 100_000)]);
        assert_eq!(reading(&[luminance, red, green]), Some(Reading::Clipped(0.02)));
    }

    #[test]
    fn no_samples_no_reading() {
        assert_eq!(reading(&[]), None);
        assert_eq!(reading(&[vec![0; 256]]), None);
    }

    #[test]
    fn labels_count_thirds_of_a_stop() {
        assert_eq!(thirds(Iso, "100"), Some(0));
        assert_eq!(thirds(Iso, "125"), Some(1));
        assert_eq!(thirds(Iso, "160"), Some(2));
        assert_eq!(thirds(Iso, "6400"), Some(18));
        assert_eq!(thirds(Iso, "50"), Some(-3));
        // A wider aperture is brighter.
        assert_eq!(thirds(Av, "f1.0"), Some(0));
        assert_eq!(thirds(Av, "f2.8"), Some(-9));
        assert_eq!(thirds(Av, "f3.2"), Some(-10));
        assert_eq!(thirds(Av, "f11"), Some(-21));
        // A slower shutter is brighter.
        assert_eq!(thirds(Tv, "1\""), Some(0));
        assert_eq!(thirds(Tv, "1/125"), Some(-21));
        assert_eq!(thirds(Tv, "1/100"), Some(-20));
        assert_eq!(thirds(Tv, "1/6"), Some(-8));
        assert_eq!(thirds(Tv, "0\"8"), Some(-1));
        assert_eq!(thirds(Tv, "2\"5"), Some(4));
        assert_eq!(thirds(Tv, "30\""), Some(15));
    }

    #[test]
    fn auto_and_bulb_have_no_brightness() {
        assert_eq!(thirds(Iso, "auto"), None);
        assert_eq!(thirds(Av, "auto"), None);
        assert_eq!(thirds(Tv, "bulb"), None);
        assert_eq!(thirds(Tv, "0x99"), None);
        assert_eq!(thirds(Av, "f0"), None);
    }
}

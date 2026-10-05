//! Ratchet catch-glitch filter tests.

use super::*;

/// A fraction of a notch: ordinary high-resolution motion, which is never
/// the glitch.
const FRACTION: f64 = 0.25;
/// One whole notch: a click in ratchet mode, or the glitch's shape.
const NOTCH: f64 = 1.0;
/// The glitch as captured: a whole number of notches against the motion.
const GLITCH: f64 = 2.0;
/// Long enough to flip direction many times.
const ALTERNATING_TICKS: usize = 40;

/// The weakest setting holds back one tick of a reversal.
const WEAKEST: WheelDebounceStrength = WheelDebounceStrength::MIN;
/// The default setting.
const DEFAULT: WheelDebounceStrength = WheelDebounceStrength::DEFAULT;
/// The strongest setting holds back the most ticks of a reversal.
const STRONGEST: WheelDebounceStrength = WheelDebounceStrength::MAX;

/// Feed `ticks` in order and report, for each, whether it was dropped.
fn dropped(
    filter: &mut WheelSettleFilter,
    ticks: &[f64],
    strength: WheelDebounceStrength,
) -> Vec<bool> {
    ticks
        .iter()
        .map(|&tick| filter.observe(tick, strength))
        .collect()
}

/// The first tick has nothing to contradict, so it passes.
#[test]
fn test_first_tick() {
    let mut filter = WheelSettleFilter::default();
    assert!(!filter.observe(NOTCH, WEAKEST));
}

/// Fractional motion passes in either direction.
#[test]
fn test_fractional_motion() {
    let mut filter = WheelSettleFilter::default();
    let ticks = [FRACTION, -FRACTION, FRACTION, -FRACTION];
    assert_eq!(dropped(&mut filter, &ticks, WEAKEST), [false; 4]);
}

/// The captured shape: smooth motion, then one whole-notch tick back, with
/// nothing after it.
#[test]
fn test_drop_glitch() {
    let mut filter = WheelSettleFilter::default();
    let ticks = [FRACTION, FRACTION, -GLITCH];
    assert_eq!(dropped(&mut filter, &ticks, WEAKEST), [false, false, true]);
}

/// The glitch stays dropped when the original direction resumes.
#[test]
fn test_glitch_then_resume() {
    let mut filter = WheelSettleFilter::default();
    let ticks = [FRACTION, FRACTION, -GLITCH, FRACTION];
    let expected = [false, false, true, false];
    assert_eq!(dropped(&mut filter, &ticks, WEAKEST), expected);
}

/// A fast flick in the ongoing direction is ordinary motion.
#[test]
fn test_fast_flick() {
    let mut filter = WheelSettleFilter::default();
    let ticks = [FRACTION, GLITCH];
    assert_eq!(dropped(&mut filter, &ticks, WEAKEST), [false, false]);
}

/// The glitch is always a whole notch, so a fractional reversal is real.
#[test]
fn test_fractional_reversal() {
    let mut filter = WheelSettleFilter::default();
    let ticks = [FRACTION, -FRACTION];
    assert_eq!(dropped(&mut filter, &ticks, WEAKEST), [false, false]);
}

/// Real motion right after a dropped glitch is judged against the last real
/// tick, not against the glitch.
#[test]
fn test_reversal_after_glitch() {
    let mut filter = WheelSettleFilter::default();
    let ticks = [FRACTION, FRACTION, -GLITCH, -FRACTION, -FRACTION];
    let expected = [false, false, true, false, false];
    assert_eq!(dropped(&mut filter, &ticks, WEAKEST), expected);
}

/// The glitch can recur within one gesture.
#[test]
fn test_drop_second_glitch() {
    let mut filter = WheelSettleFilter::default();
    let ticks = [FRACTION, -GLITCH, FRACTION, -GLITCH, FRACTION];
    let expected = [false, true, false, true, false];
    assert_eq!(dropped(&mut filter, &ticks, WEAKEST), expected);
}

/// In ratchet mode every real tick is a whole notch, the glitch's shape. The
/// first tick of a real direction change is held back, the next one confirms
/// it, and the direction never latches.
#[test]
fn test_ratchet_mode_reversal() {
    let mut filter = WheelSettleFilter::default();
    let ticks = [NOTCH, NOTCH, -NOTCH, -NOTCH, -NOTCH, NOTCH, NOTCH];
    let expected = [false, false, true, false, false, true, false];
    assert_eq!(dropped(&mut filter, &ticks, WEAKEST), expected);
}

/// The strongest setting holds back the most ticks of a reversal.
#[test]
fn test_strongest_setting() {
    let mut filter = WheelSettleFilter::default();
    filter.observe(NOTCH, STRONGEST);
    let held = STRONGEST.required_confirmations();
    for tick in 0..held {
        assert!(
            filter.observe(-NOTCH, STRONGEST),
            "tick {tick} of the reversal is held back"
        );
    }
    assert!(
        !filter.observe(-NOTCH, STRONGEST),
        "the reversal is trusted"
    );
}

/// A reversal that ends before it is trusted stays dropped.
#[test]
fn test_unsustained_reversal() {
    let mut filter = WheelSettleFilter::default();
    filter.observe(NOTCH, STRONGEST);
    let short_of_trust = STRONGEST.required_confirmations() - 1;
    for _ in 0..short_of_trust {
        assert!(filter.observe(-NOTCH, STRONGEST));
    }
    assert!(
        !filter.observe(NOTCH, STRONGEST),
        "the original direction resumes before the reversal is trusted"
    );
}

/// Requiring an unbroken run of confirmations would let one direction
/// through forever and block the other entirely.
#[test]
fn test_rapid_alternation() {
    let mut filter = WheelSettleFilter::default();
    let ticks: Vec<f64> = (0..ALTERNATING_TICKS)
        .map(|index| if index % 2 == 0 { NOTCH } else { -NOTCH })
        .collect();
    let drops = dropped(&mut filter, &ticks, STRONGEST);
    let passed_up = ticks
        .iter()
        .zip(&drops)
        .any(|(&tick, &dropped)| tick > 0.0 && !dropped);
    let passed_down = ticks
        .iter()
        .zip(&drops)
        .any(|(&tick, &dropped)| tick < 0.0 && !dropped);
    assert!(passed_up, "upward ticks get through");
    assert!(passed_down, "downward ticks get through");
}

/// An old, isolated glitch must not combine with a much later one to flip
/// the trusted direction.
#[test]
fn test_stale_reversal_cleared() {
    let mut filter = WheelSettleFilter::default();
    let ticks = [NOTCH, -NOTCH, NOTCH, NOTCH, -NOTCH, NOTCH];
    let expected = [false, true, false, false, true, false];
    assert_eq!(dropped(&mut filter, &ticks, DEFAULT), expected);
}

/// A tick with no movement leaves the history unchanged.
#[test]
fn test_zero_tick() {
    let mut filter = WheelSettleFilter::default();
    let ticks = [FRACTION, 0.0, -GLITCH];
    assert_eq!(dropped(&mut filter, &ticks, WEAKEST), [false, false, true]);
}

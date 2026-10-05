//! Filters the magnetic-ratchet catch-glitch of MagSpeed wheels: while the
//! wheel spins freely, the ratchet can re-engage mid-rotation and report a
//! discrete tick in the wrong direction, most often at the end of a gesture
//! as the finger lifts off the wheel.
//!
//! Recorded on an MX Master 4, every tick coming from the mouse itself rather
//! than from software: ordinary hi-res reporting is an always-fractional
//! stream, while each glitch is an *exact* whole number of notches, opposite
//! the motion around it, after which the real motion continues as if nothing
//! happened.
//!
//! Magnitude alone cannot tell the glitch from a genuine click: in ratchet
//! (detent) mode every real tick, in either direction, is also a whole notch.
//! The two differ only in what follows — a genuine direction change keeps
//! going, the glitch does not. So a whole-notch reversal is held as a
//! *challenger* until `required_confirmations` further whole-notch ticks in
//! its direction have accumulated. A fractional reversal is real motion and
//! is trusted at once.
//!
//! Confirmations accumulate across interruptions, and are discarded only once
//! the trusted direction sustains `required_confirmations` consecutive ticks
//! of its own. Requiring an unbroken run instead would never let fast genuine
//! back-and-forth scrolling (each burst shorter than the threshold) flip, so
//! one direction would be blocked outright; accumulating makes such
//! alternation net out, one held tick per flip. A lone trailing glitch has
//! nothing to accumulate with and stays dropped.
//!
//! Cost: in detent mode, the first `required_confirmations` ticks of each
//! genuine direction change are dropped. The strength setting sets
//! `required_confirmations`.
//!
//! It runs per mouse, on that mouse's captured main wheel, so one wheel's
//! ticks never count toward another's.

use openlogi_core::config::WheelDebounceStrength;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sign {
    Positive,
    Negative,
}

impl Sign {
    fn of(value: f64) -> Option<Self> {
        if value > 0.0 {
            Some(Self::Positive)
        } else if value < 0.0 {
            Some(Self::Negative)
        } else {
            None
        }
    }
}

/// A genuine hi-res sample landing on an exact integer is not observed in
/// practice (the device's real granularity is far finer), so this tolerance
/// only absorbs `f64` division rounding, not real ambiguity.
const WHOLE_NOTCH_EPSILON: f64 = 1e-6;

fn is_whole_notch_count(magnitude: f64) -> bool {
    (magnitude - magnitude.round()).abs() < WHOLE_NOTCH_EPSILON
}

/// Rolling state for one captured wheel.
#[derive(Default)]
pub(super) struct WheelSettleFilter {
    /// The direction currently trusted as real motion.
    last_real_sign: Option<Sign>,
    /// The direction challenging `last_real_sign`, with its accumulated
    /// confirmations. A whole-notch tick back in the trusted direction does
    /// not by itself discard them; see `trusted_streak`.
    challenger_sign: Option<Sign>,
    challenger_confirmations: u32,
    /// Consecutive ticks matching `last_real_sign` since it was last
    /// challenged. Reaching `required_confirmations` proves the trusted
    /// direction freshly dominant and discards any stale challenger, so an
    /// old, unrelated glitch cannot combine with a much later one.
    trusted_streak: u32,
}

impl WheelSettleFilter {
    /// Whether one signed tick is the ratchet catch-glitch and must be
    /// suppressed outright — not smoothed, scaled, or re-injected.
    ///
    /// `strength` sets how many whole-notch ticks opposing the trusted
    /// direction must accumulate after the first before the direction flips.
    pub(super) fn observe(&mut self, magnitude: f64, strength: WheelDebounceStrength) -> bool {
        let required_confirmations = strength.required_confirmations();
        let Some(sign) = Sign::of(magnitude) else {
            return false;
        };

        if self.last_real_sign == Some(sign) {
            self.trusted_streak += 1;
            if self.trusted_streak >= required_confirmations {
                self.challenger_sign = None;
                self.challenger_confirmations = 0;
            }
            return false;
        }

        if self.last_real_sign.is_none() || !is_whole_notch_count(magnitude) {
            // No established direction yet, or fractional hi-res motion: the
            // glitch is always a whole notch, so this is real.
            self.last_real_sign = Some(sign);
            self.trusted_streak = 1;
            self.challenger_sign = None;
            self.challenger_confirmations = 0;
            return false;
        }

        // A whole-notch tick opposing the trusted direction.
        self.trusted_streak = 0;
        if self.challenger_sign == Some(sign) {
            self.challenger_confirmations += 1;
        } else {
            self.challenger_sign = Some(sign);
            self.challenger_confirmations = 0;
        }
        if self.challenger_confirmations >= required_confirmations {
            self.last_real_sign = Some(sign);
            self.trusted_streak = 1;
            self.challenger_sign = None;
            self.challenger_confirmations = 0;
            false
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests;

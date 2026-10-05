//! Main-wheel movement for one captured session's input dispatcher.
//!
//! A captured main wheel reports in its own units. Each report is converted
//! once, at the input boundary, into the ratchet notches the wheel physically
//! moved, and re-synthesised with the device's own settings.

use std::num::NonZeroU8;

use openlogi_core::scroll::ScrollDelta;

use crate::capture_plan::MainWheelDispatch;

/// One captured main-wheel movement in ratchet notches, as the wheel
/// physically moved; positive is away from the user.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MainWheelMovement {
    notches: f64,
}

impl MainWheelMovement {
    /// Convert one report's units into notches.
    pub(super) fn from_units(delta: i16, units_per_notch: NonZeroU8) -> Self {
        Self {
            notches: f64::from(delta) / f64::from(units_per_notch.get()),
        }
    }

    /// The scroll to re-synthesise with the device's own settings: inverted
    /// when configured, scaled by its sensitivity.
    pub(super) fn scroll(self, wheel: MainWheelDispatch) -> ScrollDelta {
        let notches = if wheel.inverted {
            -self.notches
        } else {
            self.notches
        };
        ScrollDelta::wheel_ticks(0.0, notches * wheel.sensitivity.scroll_multiplier())
    }
}

#[cfg(test)]
mod tests;

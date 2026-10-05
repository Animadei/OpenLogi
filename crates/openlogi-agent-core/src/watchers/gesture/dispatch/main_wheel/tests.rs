//! Main-wheel movement conversion tests.

use openlogi_core::config::VerticalScrollSensitivity;

use super::*;

/// A high-resolution scale of 15 units per ratchet notch.
const UNITS_PER_NOTCH: i16 = 15;

fn units_per_notch() -> NonZeroU8 {
    let units = u8::try_from(UNITS_PER_NOTCH).expect("the scale fits in a byte");
    NonZeroU8::new(units).expect("the scale is not zero")
}

/// A captured wheel re-synthesised at `sensitivity`, not inverted.
fn wheel_at(sensitivity: VerticalScrollSensitivity) -> MainWheelDispatch {
    MainWheelDispatch {
        sensitivity,
        inverted: false,
    }
}

/// The vertical distance of a re-synthesised scroll, in notches.
fn vertical(delta: ScrollDelta) -> f64 {
    let ScrollDelta::WheelTicks { x, y } = delta else {
        panic!("expected wheel ticks");
    };
    assert_distance(x, 0.0);
    y
}

fn assert_distance(actual: f64, expected: f64) {
    const EPSILON: f64 = 1.0e-12;
    assert!(
        (actual - expected).abs() < EPSILON,
        "{actual} != {expected}"
    );
}

/// One notch of high-resolution units scrolls one notch.
#[test]
fn test_one_notch() {
    let movement = MainWheelMovement::from_units(UNITS_PER_NOTCH, units_per_notch());
    let scroll = movement.scroll(wheel_at(VerticalScrollSensitivity::DEFAULT));
    assert_distance(vertical(scroll), 1.0);
}

/// A notch-resolution report arrives with a scale of one unit per notch.
#[test]
fn test_notch_resolution() {
    let two_notches_down = -2;
    let movement = MainWheelMovement::from_units(two_notches_down, NonZeroU8::MIN);
    let scroll = movement.scroll(wheel_at(VerticalScrollSensitivity::DEFAULT));
    assert_distance(vertical(scroll), -2.0);
}

/// The firmware does not invert a captured wheel, so the re-synthesis does.
#[test]
fn test_inverted() {
    let inverted = MainWheelDispatch {
        inverted: true,
        ..wheel_at(VerticalScrollSensitivity::DEFAULT)
    };
    let movement = MainWheelMovement::from_units(UNITS_PER_NOTCH, units_per_notch());
    let scroll = movement.scroll(inverted);
    assert_distance(vertical(scroll), -1.0);
}

/// Sensitivity scales the scrolled distance.
#[test]
fn test_sensitivity() {
    let doubled_raw = VerticalScrollSensitivity::DEFAULT.into_inner() * 2;
    let doubled = VerticalScrollSensitivity::try_new(doubled_raw).expect("in range");
    let movement = MainWheelMovement::from_units(UNITS_PER_NOTCH, units_per_notch());
    let scroll = movement.scroll(wheel_at(doubled));
    assert_distance(vertical(scroll), 2.0);
}

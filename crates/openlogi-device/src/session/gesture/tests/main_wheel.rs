//! Main-wheel capture: arming, the hand-back, and the input each report
//! stands for.

use super::fake_mouse::FakeMouse;
use super::*;
use crate::hires_wheel::{NativeWheelMode, WheelMode};

/// High-resolution units per notch the fake wheel reports.
const UNITS_PER_NOTCH: u8 = 15;
/// A wheel that reports no notch scale at all.
const NO_UNITS_PER_NOTCH: u8 = 0;
/// Any movement.
const MOVEMENT: i16 = -7;

fn units_per_notch() -> NonZeroU8 {
    NonZeroU8::new(UNITS_PER_NOTCH).expect("the fake wheel reports a scale")
}

/// The mode the fake wheel starts in: native, high resolution, not inverted.
fn native_high_resolution() -> WheelMode {
    WheelMode::native(NativeWheelMode {
        resolution: ScrollResolution::High,
        direction: NativeDirection::Default,
    })
}

/// A capture of the main wheel alone, handed back as `spec` configures.
fn main_wheel_capture(spec: MainWheelSpec) -> CaptureSpec {
    CaptureSpec {
        main_wheel: Some(spec),
        ..CaptureSpec::default()
    }
}

/// Capture diverts the wheel at its own resolution, and again after a wake.
#[tokio::test]
async fn test_divert_and_rearm() {
    let mouse = FakeMouse::with_main_wheel(UNITS_PER_NOTCH, native_high_resolution());
    let capture = main_wheel_capture(MainWheelSpec::default());
    let (_channel, armed) = mouse.arm(&capture).await;
    armed.rearm().await;

    let diverted = WheelMode::diverted(ScrollResolution::High);
    assert_eq!(mouse.main_wheel_mode_writes(), [diverted, diverted]);
    let armed_scale = armed.main_wheel.as_ref().map(|wheel| wheel.units_per_notch);
    assert_eq!(armed_scale, Some(units_per_notch()));
}

/// The hand-back restores the configured native mode.
#[tokio::test]
async fn test_restore_native_mode() {
    let configured = MainWheelSpec {
        resolution: None,
        direction: Some(NativeDirection::Inverted),
    };
    let mouse = FakeMouse::with_main_wheel(UNITS_PER_NOTCH, native_high_resolution());
    let (channel, armed) = mouse.arm(&main_wheel_capture(configured)).await;

    // The hand-back goes through whichever channel inventory publishes now.
    let replacement =
        FakeMouse::with_main_wheel(UNITS_PER_NOTCH, WheelMode::diverted(ScrollResolution::High));
    let outcome = replacement.hand_back(armed, channel).await;

    assert!(matches!(outcome, CaptureSessionOutcome::Restored));
    let handed_back = WheelMode::native(NativeWheelMode {
        resolution: ScrollResolution::High,
        direction: NativeDirection::Inverted,
    });
    assert_eq!(replacement.main_wheel_mode_writes(), [handed_back]);
}

/// A wheel reporting no notch scale stays native.
#[tokio::test]
async fn test_zero_scale_stays_native() {
    let mouse = FakeMouse::with_main_wheel(NO_UNITS_PER_NOTCH, native_high_resolution());
    let capture = main_wheel_capture(MainWheelSpec::default());
    let (_channel, armed) = mouse.arm(&capture).await;

    assert!(armed.main_wheel.is_none());
    assert_eq!(
        mouse.main_wheel_mode_writes(),
        [],
        "the wheel is never diverted"
    );
}

/// A high-resolution report carries the wheel's scale.
#[test]
fn test_high_resolution_input() {
    let movement = hires_wheel::WheelMovement {
        delta: MOVEMENT,
        high_resolution: true,
    };
    let expected = CapturedInput::MainWheelScroll {
        delta: MOVEMENT,
        units_per_notch: units_per_notch(),
    };
    assert_eq!(
        main_wheel_input(movement, units_per_notch()),
        Some(expected)
    );
}

/// A notch-resolution report already counts whole notches, whatever the
/// wheel's high-resolution scale.
#[test]
fn test_notch_resolution_input() {
    let movement = hires_wheel::WheelMovement {
        delta: MOVEMENT,
        high_resolution: false,
    };
    let expected = CapturedInput::MainWheelScroll {
        delta: MOVEMENT,
        units_per_notch: NonZeroU8::MIN,
    };
    assert_eq!(
        main_wheel_input(movement, units_per_notch()),
        Some(expected)
    );
}

/// A report without movement is no input.
#[test]
fn test_no_movement() {
    let still = hires_wheel::WheelMovement {
        delta: 0,
        high_resolution: true,
    };
    assert_eq!(main_wheel_input(still, units_per_notch()), None);
}

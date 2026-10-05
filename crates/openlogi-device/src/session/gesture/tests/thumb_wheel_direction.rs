//! The thumb wheel's invert flag: what arming writes for each pairing of
//! diversion and inversion, the same again after a wake, and the hand-back.

use super::fake_mouse::FakeMouse;
use super::*;
use crate::thumbwheel::{
    ReportingMode, ThumbwheelInfo, ThumbwheelReporting, WheelDirection, WheelResolution,
};

/// `getThumbwheelInfo` `default_dir` of a model whose un-inverted positive
/// rotation is toward the back of the device.
const POSITIVE_IS_BACKWARD: u8 = 0;
/// `default_dir` of a model whose positive rotation is toward the front.
const POSITIVE_IS_FORWARD: u8 = 1;

/// A mouse whose thumb wheel counts positive as `default_dir` says.
fn thumb_wheel_mouse(default_dir: u8) -> FakeMouse {
    FakeMouse::with_thumbwheel(ThumbwheelInfo {
        resolution: WheelResolution::UNKNOWN,
        default_dir,
        supports_single_tap: true,
    })
}

/// Invert the thumb wheel and leave it to the OS.
fn inverted_native() -> CaptureSpec {
    CaptureSpec {
        thumbwheel_direction: WheelDirection::Inverted,
        ..CaptureSpec::default()
    }
}

/// Divert the thumb wheel in `direction`.
fn diverted(direction: WheelDirection) -> CaptureSpec {
    CaptureSpec {
        capture_thumbwheel: true,
        thumbwheel_direction: direction,
        ..CaptureSpec::default()
    }
}

fn reporting(mode: ReportingMode, direction: WheelDirection) -> ThumbwheelReporting {
    ThumbwheelReporting { mode, direction }
}

/// The one reporting arming `spec` writes to a wheel counting as `default_dir`.
async fn armed_reporting(default_dir: u8, spec: &CaptureSpec) -> ThumbwheelReporting {
    let mouse = thumb_wheel_mouse(default_dir);
    let _armed = mouse.arm(spec).await;
    let writes = mouse.thumbwheel_reporting_writes();
    assert_eq!(writes.len(), 1, "arming writes the wheel's reporting once");
    writes[0]
}

/// The firmware inverts native scroll itself, whichever way the model counts.
#[tokio::test]
async fn test_invert_native() {
    let inverted = reporting(ReportingMode::Native, WheelDirection::Inverted);
    for default_dir in [POSITIVE_IS_BACKWARD, POSITIVE_IS_FORWARD] {
        let armed = armed_reporting(default_dir, &inverted_native()).await;
        assert_eq!(armed, inverted, "default_dir {default_dir}");
    }
}

/// Diverting normalises rotation so that positive is forward.
#[tokio::test]
async fn test_divert() {
    let spec = diverted(WheelDirection::Default);
    assert_eq!(
        armed_reporting(POSITIVE_IS_BACKWARD, &spec).await,
        reporting(ReportingMode::Diverted, WheelDirection::Inverted)
    );
    assert_eq!(
        armed_reporting(POSITIVE_IS_FORWARD, &spec).await,
        reporting(ReportingMode::Diverted, WheelDirection::Default)
    );
}

/// Inverting a diverted wheel flips its normalised rotation, so both its
/// re-synthesised scroll and its two bound directions swap.
#[tokio::test]
async fn test_invert_diverted() {
    let spec = diverted(WheelDirection::Inverted);
    assert_eq!(
        armed_reporting(POSITIVE_IS_BACKWARD, &spec).await,
        reporting(ReportingMode::Diverted, WheelDirection::Default)
    );
    assert_eq!(
        armed_reporting(POSITIVE_IS_FORWARD, &spec).await,
        reporting(ReportingMode::Diverted, WheelDirection::Inverted)
    );
}

/// A wake re-arms the inversion, and the hand-back clears it.
#[tokio::test]
async fn test_rearm_and_restore() {
    let mouse = thumb_wheel_mouse(POSITIVE_IS_FORWARD);
    let (channel, armed) = mouse.arm(&inverted_native()).await;
    armed.rearm().await;
    let outcome = mouse.hand_back(armed, channel).await;

    assert!(matches!(outcome, CaptureSessionOutcome::Restored));
    let inverted = reporting(ReportingMode::Native, WheelDirection::Inverted);
    let handed_back = reporting(ReportingMode::Native, WheelDirection::Default);
    assert_eq!(
        mouse.thumbwheel_reporting_writes(),
        [inverted, inverted, handed_back]
    );
}

/// A wheel neither diverted nor inverted is left alone.
#[tokio::test]
async fn test_untouched() {
    let mouse = thumb_wheel_mouse(POSITIVE_IS_FORWARD);
    let (_channel, armed) = mouse.arm(&CaptureSpec::default()).await;

    assert!(armed.thumb.is_none());
    assert_eq!(mouse.thumbwheel_reporting_writes(), []);
}

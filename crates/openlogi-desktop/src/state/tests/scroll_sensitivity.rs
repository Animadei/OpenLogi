//! Per-mouse vertical sensitivity against the app-wide value it overrides.

use super::*;

/// A mouse keeps its own vertical sensitivity until it is set back to the
/// app-wide value.
#[test]
fn test_mouse_sensitivity() {
    let mut state = state_with_a_known_mouse();
    let key = state
        .current_record()
        .map(DeviceRecord::device_key)
        .expect("the fixture selects its mouse");
    let stored = |state: &AppState| {
        state
            .config
            .devices
            .get(key.as_str())
            .and_then(|device| device.vertical_scroll_sensitivity)
    };

    let _ = state.commit_device_vertical_scroll_sensitivity(&key, VerticalScrollSensitivity::MAX);
    assert_eq!(stored(&state), Some(VerticalScrollSensitivity::MAX));
    assert_eq!(
        state.device_vertical_scroll_sensitivity(key.as_str()),
        VerticalScrollSensitivity::MAX
    );

    let _ = state.commit_vertical_scroll_sensitivity(VerticalScrollSensitivity::MIN);
    assert_eq!(
        state.device_vertical_scroll_sensitivity(key.as_str()),
        VerticalScrollSensitivity::MAX,
        "the global never overrides a mouse's own value"
    );

    let _ = state.commit_device_vertical_scroll_sensitivity(&key, VerticalScrollSensitivity::MIN);
    assert_eq!(
        stored(&state),
        None,
        "landing on the global value goes back to following it"
    );
}

/// With the app-wide switch off, mice without their own value scroll at the
/// default.
#[test]
fn test_global_sensitivity_off() {
    let mut state = state_with_a_known_mouse();
    let key = state
        .current_record()
        .map(DeviceRecord::device_key)
        .expect("the fixture selects its mouse");
    let _ = state.commit_vertical_scroll_sensitivity(VerticalScrollSensitivity::MIN);
    assert_eq!(
        state.device_vertical_scroll_sensitivity(key.as_str()),
        VerticalScrollSensitivity::MIN
    );

    let _ = state.commit_vertical_scroll_sensitivity_enabled(false);
    assert_eq!(
        state.device_vertical_scroll_sensitivity(key.as_str()),
        VerticalScrollSensitivity::DEFAULT
    );
    assert_eq!(
        state.app_settings().vertical_scroll_sensitivity,
        VerticalScrollSensitivity::MIN,
        "the global value is kept for when it is turned back on"
    );

    let _ = state.commit_thumbwheel_sensitivity(ThumbwheelSensitivity::MAX);
    let _ = state.commit_thumbwheel_sensitivity_enabled(false);
    assert_eq!(
        state.device_thumbwheel_sensitivity(key.as_str()),
        ThumbwheelSensitivity::DEFAULT
    );
}

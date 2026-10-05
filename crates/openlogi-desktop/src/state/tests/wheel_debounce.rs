//! Per-mouse ratchet debounce: saved for the selected mouse, and only on a
//! mouse whose main wheel OpenLogi can capture.

use super::*;

fn state_with_a_hires_wheel_mouse() -> AppState {
    state_with_mouse_capabilities(|capabilities| capabilities.hires_wheel = true)
}

/// The ratchet debounce is off until turned on, and keeps its strength while
/// off.
#[test]
fn test_wheel_debounce() {
    let mut state = state_with_a_hires_wheel_mouse();
    assert_eq!(state.current_wheel_debounce(), WheelDebounce::default());

    let _ = state.commit_wheel_debounce_strength(WheelDebounceStrength::MAX);
    let _ = state.commit_wheel_debounce_enabled(true);
    let turned_on = WheelDebounce {
        enabled: true,
        strength: WheelDebounceStrength::MAX,
    };
    assert_eq!(state.current_wheel_debounce(), turned_on);
    assert_eq!(state.config.wheel_debounce(KNOWN_MOUSE_KEY), turned_on);

    let _ = state.commit_wheel_debounce_enabled(false);
    let turned_off = WheelDebounce {
        enabled: false,
        ..turned_on
    };
    assert_eq!(
        state.current_wheel_debounce(),
        turned_off,
        "the strength is kept while the filter is off"
    );
}

/// A mouse without a high-resolution wheel keeps the ratchet debounce off.
#[test]
fn test_no_hires_wheel() {
    let mut state = state_with_a_known_mouse();
    let _ = state.commit_wheel_debounce_enabled(true);
    assert_eq!(state.current_wheel_debounce(), WheelDebounce::default());
    assert_eq!(
        state.config.wheel_debounce(KNOWN_MOUSE_KEY),
        WheelDebounce::default()
    );
}

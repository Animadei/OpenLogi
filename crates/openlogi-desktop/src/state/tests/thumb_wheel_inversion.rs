//! Per-mouse thumb-wheel inversion: saved for the selected mouse, and only on a
//! mouse with a HID++ thumb wheel.

use super::*;

/// Thumb-wheel inversion is saved for a mouse with a thumb wheel.
#[test]
fn test_invert_thumbwheel() {
    let mut state = state_with_mouse_capabilities(|capabilities| capabilities.thumbwheel = true);
    assert!(!state.current_invert_thumbwheel());

    let _ = state.commit_invert_thumbwheel(true);
    assert!(state.current_invert_thumbwheel());
    assert!(state.config.invert_thumbwheel(KNOWN_MOUSE_KEY));
    assert!(
        !state.current_invert_scroll(),
        "the main wheel keeps its own direction"
    );

    let _ = state.commit_invert_thumbwheel(false);
    assert!(!state.current_invert_thumbwheel());
}

/// A mouse without a thumb wheel cannot be inverted.
#[test]
fn test_no_thumbwheel() {
    let mut state = state_with_a_known_mouse();
    let _ = state.commit_invert_thumbwheel(true);
    assert!(!state.current_invert_thumbwheel());
    assert!(!state.config.invert_thumbwheel(KNOWN_MOUSE_KEY));
}

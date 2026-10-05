//! A setting's on/off row: its title and what it does, beside its switch.
//!
//! The Scrolling card's switches share this one shape. A setting the selected
//! device cannot apply still shows its stored value, with the switch disabled
//! and marked unavailable and the description saying why.

use gpui::{App, Div, ParentElement, SharedString, Styled, div};
use gpui_component::{Disableable as _, Selectable as _, h_flex, v_flex};

use crate::ui::components::Toggle;
use crate::ui::theme::{Palette, Typography as _};

/// One setting's switch row.
pub(crate) struct ToggleRow {
    /// The switch's element id.
    pub(crate) id: &'static str,
    pub(crate) title: SharedString,
    /// What the setting does.
    pub(crate) description: SharedString,
    /// Why the selected device cannot have the setting, shown instead of
    /// `description`.
    pub(crate) unsupported: SharedString,
    pub(crate) state: ToggleState,
}

/// What a switch shows. The stored value is independent of support — a value
/// stored for a device that cannot apply it still renders, checked and
/// disabled — so these are two named fields, not a sum type.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ToggleState {
    /// The stored value.
    pub(crate) on: bool,
    /// Whether the selected device can apply it.
    pub(crate) supported: bool,
}

impl ToggleRow {
    /// Render the row. `commit` gets the new value when the user flips the
    /// switch.
    pub(crate) fn render(self, pal: Palette, commit: impl Fn(bool, &mut App) + 'static) -> Div {
        let ToggleState { on, supported } = self.state;
        let description = if supported {
            self.description
        } else {
            self.unsupported
        };
        let title = div()
            .text_body()
            .text_color(pal.text_primary)
            .child(self.title);
        let description = div()
            .text_caption()
            .text_color(pal.text_muted)
            .child(description);
        let unavailable = (!supported).then(|| tr!("common.unavailable"));
        let switch = Toggle::new(self.id)
            .selected(on)
            .disabled(!supported)
            .label(unavailable)
            .on_change(move |on, _window, cx| commit(*on, cx));
        h_flex()
            .justify_between()
            .items_center()
            .gap_4()
            .child(v_flex().child(title).child(description))
            .child(switch)
    }
}

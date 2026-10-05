//! Per-mouse main-wheel controls for the Scrolling card.
//!
//! The vertical sensitivity here belongs to the selected mouse alone; mice
//! without their own follow the app-wide setting in Settings → General while
//! it is turned on. Applying it needs the mouse's main wheel captured over
//! HID++, so it is offered only on mice that expose a high-resolution wheel.
//!
//! The ratchet debounce drops the one tick in the wrong direction a MagSpeed
//! wheel can send as its ratchet catches it. It also runs on the captured
//! wheel, and is off until turned on for a mouse.

use gpui::{
    App, Context, Div, IntoElement, ParentElement, Render, SharedString, Styled, Subscription,
    Window, div, rgb,
};
use gpui_component::{h_flex, slider::Slider, v_flex};
use openlogi_core::config::{VerticalScrollSensitivity, WheelDebounce, WheelDebounceStrength};

use super::smartshift::disabled_track;
use crate::state::{AppState, DeviceRecord, StateEvent, StateEvents};
use crate::ui::commit_slider::{CommitSlider, SliderRange};
use crate::ui::section::section_label;
use crate::ui::theme::{self, ACCENT_BLUE, Palette, Typography as _};
use crate::ui::toggle_row::{ToggleRow, ToggleState};

pub struct MainWheelPanel {
    /// The per-device vertical sensitivity slider (device override; devices
    /// without one follow the app-wide default from Settings → General).
    vertical_sensitivity: CommitSlider<VerticalScrollSensitivity>,
    /// The per-device ratchet debounce strength slider.
    wheel_debounce_strength: CommitSlider<WheelDebounceStrength>,
    _state_obs: Subscription,
}

impl MainWheelPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let vertical_sensitivity = CommitSlider::new(
            SliderRange::new(
                VerticalScrollSensitivity::MIN,
                VerticalScrollSensitivity::MAX,
            ),
            VerticalScrollSensitivity::DEFAULT,
            cx,
            |_, sensitivity, cx| {
                AppState::apply(cx, |state| {
                    state
                        .current_record()
                        .map(DeviceRecord::device_key)
                        .map_or_else(StateEvents::none, |key| {
                            state.commit_device_vertical_scroll_sensitivity(&key, sensitivity)
                        })
                });
            },
        );
        let wheel_debounce_strength = CommitSlider::new(
            SliderRange::new(WheelDebounceStrength::MIN, WheelDebounceStrength::MAX),
            WheelDebounceStrength::DEFAULT,
            cx,
            |_, strength, cx| {
                AppState::apply(cx, |state| state.commit_wheel_debounce_strength(strength));
            },
        );
        let state_obs = AppState::repaint_on(cx, |event| {
            matches!(
                event,
                StateEvent::DeviceConfigChanged(_) | StateEvent::SettingsChanged
            )
        });
        Self {
            vertical_sensitivity,
            wheel_debounce_strength,
            _state_obs: state_obs,
        }
    }

    /// The per-device vertical sensitivity row. Re-seats the thumb on a device
    /// switch or external config change, never mid-drag.
    fn vertical_sensitivity_row(
        &mut self,
        facts: &MainWheelFacts,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let committed = facts.vertical_sensitivity;
        self.vertical_sensitivity.sync(committed, window, cx);
        let shown = self.vertical_sensitivity.shown(committed);

        let slider = facts
            .supported
            .then(|| Slider::new(self.vertical_sensitivity.slider()).horizontal());
        let caption = if facts.supported {
            tr!("pointer.device_vertical_scroll_sensitivity_description")
        } else {
            tr!("pointer.device_vertical_scroll_sensitivity_unsupported")
        };
        let row = SliderRow {
            label: tr!("pointer.vertical_scroll_sensitivity"),
            value: shown.to_string().into(),
            slider,
            caption,
        };
        row.render(theme::palette(cx))
    }

    /// The ratchet debounce strength row, greyed out while the filter is off.
    fn wheel_debounce_strength_row(
        &mut self,
        facts: &MainWheelFacts,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let committed = facts.wheel_debounce.strength;
        self.wheel_debounce_strength.sync(committed, window, cx);
        let shown = self.wheel_debounce_strength.shown(committed);

        let adjustable = facts.supported && facts.wheel_debounce.enabled;
        let slider =
            adjustable.then(|| Slider::new(self.wheel_debounce_strength.slider()).horizontal());
        let row = SliderRow {
            label: tr!("pointer.wheel_debounce_strength"),
            value: shown.to_string().into(),
            slider,
            caption: tr!("pointer.wheel_debounce_strength_description"),
        };
        row.render(theme::palette(cx))
    }
}

/// The ratchet debounce switch.
fn wheel_debounce_toggle_row(facts: &MainWheelFacts, pal: Palette) -> Div {
    let row = ToggleRow {
        id: "wheel-debounce-toggle",
        title: tr!("pointer.wheel_debounce_enabled"),
        description: tr!("pointer.wheel_debounce_enabled_description"),
        unsupported: tr!("pointer.wheel_debounce_unsupported"),
        state: ToggleState {
            on: facts.wheel_debounce.enabled,
            supported: facts.supported,
        },
    };
    row.render(pal, |enabled, cx| {
        AppState::apply(cx, |state| state.commit_wheel_debounce_enabled(enabled));
    })
}

/// What the panel shows for the selected mouse.
#[derive(Default)]
struct MainWheelFacts {
    /// Its effective vertical sensitivity: its own value, or the app-wide one.
    vertical_sensitivity: VerticalScrollSensitivity,
    /// Its ratchet debounce; the strength is kept while the filter is off.
    wheel_debounce: WheelDebounce,
    /// Whether it has a wheel OpenLogi can capture to apply its own settings.
    supported: bool,
}

impl MainWheelFacts {
    fn read(cx: &App) -> Self {
        let Some(state) = AppState::try_read(cx) else {
            return Self::default();
        };
        let vertical_sensitivity = match state.current_record() {
            Some(record) => state.device_vertical_scroll_sensitivity(&record.config_key),
            None => VerticalScrollSensitivity::DEFAULT,
        };
        Self {
            vertical_sensitivity,
            wheel_debounce: state.current_wheel_debounce(),
            supported: state.current_hires_wheel_supported(),
        }
    }
}

/// One slider row: the setting's name and live value over its slider, with a
/// caption under them.
struct SliderRow {
    label: SharedString,
    value: SharedString,
    /// The slider, or `None` for a greyed-out track while it is unavailable.
    slider: Option<Slider>,
    caption: SharedString,
}

impl SliderRow {
    fn render(self, pal: Palette) -> Div {
        let value_color = if self.slider.is_some() {
            rgb(ACCENT_BLUE).into()
        } else {
            pal.text_muted
        };
        let label = section_label(self.label, pal);
        let value = div().text_body().text_color(value_color).child(self.value);
        let header = h_flex()
            .justify_between()
            .items_baseline()
            .child(label)
            .child(value);
        let control = match self.slider {
            Some(slider) => div().child(slider),
            None => disabled_track(pal),
        };
        let caption = div()
            .text_caption()
            .text_color(pal.text_muted)
            .child(self.caption);
        v_flex().gap_2().child(header).child(control).child(caption)
    }
}

impl Render for MainWheelPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let facts = MainWheelFacts::read(cx);
        let pal = theme::palette(cx);
        let vertical_sensitivity = self.vertical_sensitivity_row(&facts, window, cx);
        let debounce_switch = wheel_debounce_toggle_row(&facts, pal);
        let debounce_strength = self.wheel_debounce_strength_row(&facts, window, cx);
        v_flex()
            .gap_4()
            .w_full()
            .child(vertical_sensitivity)
            .child(debounce_switch)
            .child(debounce_strength)
    }
}

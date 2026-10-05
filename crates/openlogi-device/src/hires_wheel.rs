//! HID++ `HiResWheel` — capture a mouse's main wheel so its movement arrives as
//! HID++ events tagged with the device, instead of as native HID scroll that
//! the OS cannot attribute to any one mouse.
//!
//! Capturing is how a per-mouse wheel setting, such as its own vertical
//! sensitivity, is applied: the capture session diverts the wheel, and its
//! movement is re-synthesised with that mouse's settings. The firmware ignores
//! its invert flag while diverted, so an inverted wheel is inverted by the
//! re-synthesis instead.
//!
//! `HiResWheelFeature` installs an event listener of its own, so we re-implement
//! the three functions a capture session needs on its shared channel:
//! `getWheelCapability` (the high-resolution multiplier), `getWheelMode` and
//! `setWheelMode` (enter/leave diverted mode), and decode the unsolicited
//! `wheelMovement` event.

use std::num::NonZeroU8;
use std::sync::Arc;

use hidpp::{
    channel::HidppChannel,
    feature::{CreatableFeature as _, hires_wheel::HiResWheelFeature},
    nibble::U4,
    protocol::v20::{self, Hidpp20Error},
};
use openlogi_core::config::ScrollResolution;

/// `HiResWheel` HID++ feature ID.
pub const FEATURE_ID: u16 = HiResWheelFeature::ID;

/// `getWheelCapability` function ID.
pub(crate) const FN_GET_CAPABILITY: u8 = 0;
/// `getWheelMode` function ID.
pub(crate) const FN_GET_MODE: u8 = 1;
/// `setWheelMode` function ID.
pub(crate) const FN_SET_MODE: u8 = 2;
/// `wheelMovement` event ID.
const EV_WHEEL_MOVEMENT: u8 = 0;

/// Mode flag: report movement to software as HID++ events instead of as
/// normal scrolling.
const MODE_DIVERTED: u8 = 1 << 0;
/// Mode flag: report in high-resolution units instead of whole notches.
const MODE_HIGH_RESOLUTION: u8 = 1 << 1;
/// Mode flag: scroll the other way. The firmware applies it to normal
/// scrolling only, not to diverted movement.
const MODE_INVERTED: u8 = 1 << 2;
/// `wheelMovement` flag (byte 0): the movement is in high-resolution units.
const EVENT_HIGH_RESOLUTION: u8 = 1 << 4;

/// Characteristics returned by `getWheelCapability`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WheelCapability {
    /// High-resolution units per ratchet notch (the `multiplier`).
    pub units_per_notch: NonZeroU8,
}

/// Where a wheel reports its movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelTarget {
    /// Native HID scroll.
    Native,
    /// HID++ `wheelMovement` events.
    Diverted,
}

/// Scroll direction of native wheel reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeDirection {
    /// Report movement with the wheel's default sign.
    Default,
    /// Invert the movement sign.
    Inverted,
}

/// The wheel's native reporting mode: what a capture session hands back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeWheelMode {
    /// Reporting resolution.
    pub resolution: ScrollResolution,
    /// Reporting direction.
    pub direction: NativeDirection,
}

/// A `getWheelMode` / `setWheelMode` mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WheelMode {
    /// Where movement is reported.
    pub target: WheelTarget,
    /// Reporting resolution.
    pub resolution: ScrollResolution,
    /// Direction of native reports.
    pub direction: NativeDirection,
}

impl WheelMode {
    /// Diverted reporting at `resolution`. The firmware ignores the invert
    /// flag while diverted, so it stays clear.
    #[must_use]
    pub fn diverted(resolution: ScrollResolution) -> Self {
        Self {
            target: WheelTarget::Diverted,
            resolution,
            direction: NativeDirection::Default,
        }
    }

    /// Native reporting in `mode`.
    #[must_use]
    pub fn native(mode: NativeWheelMode) -> Self {
        Self {
            target: WheelTarget::Native,
            resolution: mode.resolution,
            direction: mode.direction,
        }
    }

    /// The resolution and direction, whichever way movement is reported.
    #[must_use]
    pub fn native_mode(self) -> NativeWheelMode {
        NativeWheelMode {
            resolution: self.resolution,
            direction: self.direction,
        }
    }

    /// Decode a mode byte.
    pub(crate) fn from_byte(byte: u8) -> Self {
        Self {
            target: if byte & MODE_DIVERTED == 0 {
                WheelTarget::Native
            } else {
                WheelTarget::Diverted
            },
            resolution: if byte & MODE_HIGH_RESOLUTION == 0 {
                ScrollResolution::Low
            } else {
                ScrollResolution::High
            },
            direction: if byte & MODE_INVERTED == 0 {
                NativeDirection::Default
            } else {
                NativeDirection::Inverted
            },
        }
    }

    /// Encode as a mode byte.
    pub(crate) fn to_byte(self) -> u8 {
        let target = match self.target {
            WheelTarget::Native => 0,
            WheelTarget::Diverted => MODE_DIVERTED,
        };
        let resolution = match self.resolution {
            ScrollResolution::Low => 0,
            ScrollResolution::High => MODE_HIGH_RESOLUTION,
        };
        let direction = match self.direction {
            NativeDirection::Default => 0,
            NativeDirection::Inverted => MODE_INVERTED,
        };
        target | resolution | direction
    }
}

/// A decoded `wheelMovement` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WheelMovement {
    /// Movement since the last report; positive is away from the user.
    pub delta: i16,
    /// Whether `delta` counts high-resolution units rather than whole
    /// ratchet notches.
    pub high_resolution: bool,
}

/// Decode a channel message into a [`WheelMovement`] when it is the
/// unsolicited HiResWheel `wheelMovement` event for
/// `(device_index, feature_index)`.
///
/// Returns `None` for request responses (`software_id != 0`), the ratchet
/// switch event, and messages from a different device or feature.
#[must_use]
pub fn decode_event(
    msg: &v20::Message,
    device_index: u8,
    feature_index: u8,
) -> Option<WheelMovement> {
    let header = msg.header();
    if header.device_index != device_index
        || header.feature_index != feature_index
        || header.software_id.to_lo() != 0
        || header.function_id.to_lo() != EV_WHEEL_MOVEMENT
    {
        return None;
    }
    let p = msg.extend_payload();
    Some(WheelMovement {
        delta: i16::from_be_bytes([p[1], p[2]]),
        high_resolution: p[0] & EVENT_HIGH_RESOLUTION != 0,
    })
}

/// `HiResWheel` accessor bound to one device + resolved feature index.
///
/// Construct with the feature index from the device's root feature
/// (`get_feature(`[`FEATURE_ID`]`)`). Cheap to clone (an `Arc` plus two indices).
#[derive(Clone)]
pub struct HiResWheel {
    chan: Arc<HidppChannel>,
    device_index: u8,
    feature_index: u8,
}

impl HiResWheel {
    /// Bind the feature to `(device_index, feature_index)` on `chan`.
    #[must_use]
    pub fn new(chan: Arc<HidppChannel>, device_index: u8, feature_index: u8) -> Self {
        Self {
            chan,
            device_index,
            feature_index,
        }
    }

    /// The feature index this accessor talks to — used to match unsolicited
    /// events in [`decode_event`].
    #[must_use]
    pub fn feature_index(&self) -> u8 {
        self.feature_index
    }

    /// Send a feature function call carrying a full long-message payload.
    async fn call(&self, function_id: u8, params: [u8; 16]) -> Result<[u8; 16], Hidpp20Error> {
        let response = self
            .chan
            .send_v20(v20::Message::Long(
                v20::MessageHeader {
                    device_index: self.device_index,
                    feature_index: self.feature_index,
                    function_id: U4::from_lo(function_id),
                    software_id: self.chan.get_sw_id(),
                },
                params,
            ))
            .await?;
        Ok(response.extend_payload())
    }

    /// Read the wheel's notch scale. A zero multiplier cannot scale movement,
    /// so it is an unsupported response.
    pub async fn get_capability(&self) -> Result<WheelCapability, Hidpp20Error> {
        let p = self.call(FN_GET_CAPABILITY, [0; 16]).await?;
        let units_per_notch = NonZeroU8::new(p[0]).ok_or(Hidpp20Error::UnsupportedResponse)?;
        Ok(WheelCapability { units_per_notch })
    }

    /// Read the wheel's reporting mode.
    pub async fn get_mode(&self) -> Result<WheelMode, Hidpp20Error> {
        let p = self.call(FN_GET_MODE, [0; 16]).await?;
        Ok(WheelMode::from_byte(p[0]))
    }

    /// Enter diverted reporting at `resolution`; wheel events then arrive on
    /// this feature index instead of moving the native scroll.
    pub async fn divert(&self, resolution: ScrollResolution) -> Result<(), Hidpp20Error> {
        self.set_mode(WheelMode::diverted(resolution)).await
    }

    /// Hand native scrolling back to the firmware in `mode`.
    pub async fn undivert(&self, mode: NativeWheelMode) -> Result<(), Hidpp20Error> {
        self.set_mode(WheelMode::native(mode)).await
    }

    async fn set_mode(&self, mode: WheelMode) -> Result<(), Hidpp20Error> {
        let mut params = [0u8; 16];
        params[0] = mode.to_byte();
        self.call(FN_SET_MODE, params).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The device and feature index the test messages are addressed to. Any
    /// values work, as long as decoding is asked for the same ones.
    const DEVICE_INDEX: u8 = 2;
    const FEATURE_INDEX: u8 = 7;
    /// `ratchetSwitch`, the feature's other event.
    const EV_RATCHET_SWITCH: u8 = 1;
    /// Unsolicited events carry software ID 0; a reply carries the ID of the
    /// request it answers.
    const EVENT_SOFTWARE_ID: u8 = 0;
    const REPLY_SOFTWARE_ID: u8 = 5;

    /// Any downward movement.
    const DOWNWARD: i16 = -45;
    /// That movement, reported at high resolution.
    const HIGH_RESOLUTION_DOWNWARD: WheelMovement = WheelMovement {
        delta: DOWNWARD,
        high_resolution: true,
    };

    /// The header of an unsolicited event `function_id` from the test
    /// device's feature.
    fn event_header(function_id: u8) -> v20::MessageHeader {
        v20::MessageHeader {
            device_index: DEVICE_INDEX,
            feature_index: FEATURE_INDEX,
            function_id: U4::from_lo(function_id),
            software_id: U4::from_lo(EVENT_SOFTWARE_ID),
        }
    }

    /// A `wheelMovement` payload carrying `movement`: the high-resolution
    /// flag in byte 0, the signed movement in bytes 1–2.
    fn movement_payload(movement: WheelMovement) -> [u8; 16] {
        let mut payload = [0u8; 16];
        if movement.high_resolution {
            payload[0] = EVENT_HIGH_RESOLUTION;
        }
        payload[1..3].copy_from_slice(&movement.delta.to_be_bytes());
        payload
    }

    /// `movement` sent as a message with `header`.
    fn movement_message(header: v20::MessageHeader, movement: WheelMovement) -> v20::Message {
        v20::Message::Long(header, movement_payload(movement))
    }

    /// `movement` as the test device reports it.
    fn movement_event(movement: WheelMovement) -> v20::Message {
        movement_message(event_header(EV_WHEEL_MOVEMENT), movement)
    }

    fn decode(msg: &v20::Message) -> Option<WheelMovement> {
        decode_event(msg, DEVICE_INDEX, FEATURE_INDEX)
    }

    #[test]
    fn test_decode_high_resolution() {
        assert_eq!(
            decode(&movement_event(HIGH_RESOLUTION_DOWNWARD)),
            Some(HIGH_RESOLUTION_DOWNWARD)
        );
    }

    #[test]
    fn test_decode_notch_resolution() {
        let movement = WheelMovement {
            delta: DOWNWARD,
            high_resolution: false,
        };
        assert_eq!(decode(&movement_event(movement)), Some(movement));
    }

    #[test]
    fn test_ignore_reply() {
        let reply = v20::MessageHeader {
            software_id: U4::from_lo(REPLY_SOFTWARE_ID),
            ..event_header(EV_WHEEL_MOVEMENT)
        };
        let movement = HIGH_RESOLUTION_DOWNWARD;
        assert_eq!(decode(&movement_message(reply, movement)), None);
    }

    #[test]
    fn test_ignore_ratchet_switch() {
        let switch = v20::Message::Long(event_header(EV_RATCHET_SWITCH), [0u8; 16]);
        assert_eq!(decode(&switch), None);
    }

    #[test]
    fn test_ignore_other_device() {
        let another_feature = v20::MessageHeader {
            feature_index: FEATURE_INDEX + 1,
            ..event_header(EV_WHEEL_MOVEMENT)
        };
        let another_device = v20::MessageHeader {
            device_index: DEVICE_INDEX + 1,
            ..event_header(EV_WHEEL_MOVEMENT)
        };
        let movement = HIGH_RESOLUTION_DOWNWARD;
        assert_eq!(decode(&movement_message(another_feature, movement)), None);
        assert_eq!(decode(&movement_message(another_device, movement)), None);
    }

    /// Diverted mode sets only the divert and high-resolution flags.
    #[test]
    fn test_encode_diverted_mode() {
        let diverted = WheelMode::diverted(ScrollResolution::High);
        assert_eq!(diverted.to_byte(), MODE_DIVERTED | MODE_HIGH_RESOLUTION);
    }

    /// Native mode keeps its high-resolution and invert flags.
    #[test]
    fn test_encode_native_mode() {
        let native = WheelMode::native(NativeWheelMode {
            resolution: ScrollResolution::High,
            direction: NativeDirection::Inverted,
        });
        assert_eq!(native.to_byte(), MODE_HIGH_RESOLUTION | MODE_INVERTED);
    }

    #[test]
    fn test_mode_round_trip() {
        for target in [WheelTarget::Native, WheelTarget::Diverted] {
            for resolution in [ScrollResolution::Low, ScrollResolution::High] {
                for direction in [NativeDirection::Default, NativeDirection::Inverted] {
                    let mode = WheelMode {
                        target,
                        resolution,
                        direction,
                    };
                    assert_eq!(WheelMode::from_byte(mode.to_byte()), mode);
                }
            }
        }
    }
}

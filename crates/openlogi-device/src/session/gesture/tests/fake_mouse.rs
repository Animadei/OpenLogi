//! A scripted mouse for capture-session tests. It answers the HID++ root
//! feature and the wheel features a test gives it, and remembers what a
//! session writes to them, so a test asserts what the session did to the wheel
//! instead of decoding bytes.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use hidpp::channel::{HidppChannel, LONG_REPORT_ID, LONG_REPORT_LENGTH};

use super::*;
use crate::hires_wheel::{self, WheelMode};
use crate::thumbwheel::{self, ThumbwheelInfo, ThumbwheelReporting};

/// Byte positions in a HID++ 2.0 report.
const REPORT_ID: usize = 0;
const FEATURE_INDEX: usize = 2;
/// Function ID in the high nibble, software ID in the low one.
const FUNCTION_AND_SOFTWARE_ID: usize = 3;
const PARAMS: usize = 4;

/// The root feature sits at index 0 on every HID++ 2.0 device.
const ROOT_FEATURE_INDEX: u8 = 0;
/// Root `getFeature`: its first two parameters name a feature, and the reply
/// starts with that feature's index.
const ROOT_GET_FEATURE: u8 = 0;
/// Root `getProtocolVersion`: the reply starts with the major version.
const ROOT_GET_PROTOCOL_VERSION: u8 = 1;
/// A HID++ 2.0 major protocol version.
const PROTOCOL_MAJOR_VERSION: u8 = 4;
/// The index root `getFeature` answers for a feature the device lacks.
const ABSENT_FEATURE_INDEX: u8 = 0;

/// Where the fake's feature table puts its main wheel.
const MAIN_WHEEL_FEATURE_INDEX: u8 = 8;
/// Where the fake's feature table puts its thumb wheel.
const THUMBWHEEL_FEATURE_INDEX: u8 = 6;

/// A mouse with the wheel features a test gives it.
#[derive(Clone)]
pub(super) struct FakeMouse {
    features: Arc<Mutex<Features>>,
}

/// The fake's features, each present only when a test asked for it.
#[derive(Default)]
struct Features {
    main_wheel: Option<MainWheelState>,
    thumbwheel: Option<ThumbwheelState>,
}

struct MainWheelState {
    /// The `getWheelCapability` multiplier: high-resolution units per notch.
    multiplier: u8,
    /// The mode `getWheelMode` reports and `setWheelMode` replaces.
    mode: WheelMode,
    /// Every mode `setWheelMode` was asked for, in order.
    mode_writes: Vec<WheelMode>,
}

struct ThumbwheelState {
    /// What `getThumbwheelInfo` reports.
    info: ThumbwheelInfo,
    /// Every `setThumbwheelReporting` request, in order.
    reporting_writes: Vec<ThumbwheelReporting>,
}

impl FakeMouse {
    /// A mouse whose only feature is a high-resolution main wheel reporting
    /// `multiplier` units per notch, currently in `mode`.
    pub(super) fn with_main_wheel(multiplier: u8, mode: WheelMode) -> Self {
        let main_wheel = MainWheelState {
            multiplier,
            mode,
            mode_writes: Vec::new(),
        };
        Self::with(Features {
            main_wheel: Some(main_wheel),
            ..Features::default()
        })
    }

    /// A mouse whose only feature is a thumb wheel reporting `info`.
    pub(super) fn with_thumbwheel(info: ThumbwheelInfo) -> Self {
        let thumbwheel = ThumbwheelState {
            info,
            reporting_writes: Vec::new(),
        };
        Self::with(Features {
            thumbwheel: Some(thumbwheel),
            ..Features::default()
        })
    }

    fn with(features: Features) -> Self {
        Self {
            features: Arc::new(Mutex::new(features)),
        }
    }

    /// A HID++ channel to this mouse.
    pub(super) async fn channel(&self) -> Arc<HidppChannel> {
        let mouse = self.clone();
        let (raw, _handle) = ScriptedRawHidChannel::with_dynamic_responder(move |request| {
            Some(mouse.reply(request))
        });
        scripted_channel(raw).await
    }

    /// Arm the controls `spec` selects on this mouse, returning the channel
    /// they were armed over and the armed controls.
    pub(super) async fn arm(&self, spec: &CaptureSpec) -> (Arc<HidppChannel>, ArmedControls) {
        let channel = self.channel().await;
        let device = Device::new(channel.clone(), crate::DIRECT_DEVICE_INDEX)
            .await
            .expect("the fake mouse answers the root feature");
        let mut armed = ArmedControls::default();
        arm_controls_into(
            &device,
            &channel,
            crate::DIRECT_DEVICE_INDEX,
            spec,
            &mut armed,
        )
        .await
        .expect("arming succeeds");
        (channel, armed)
    }

    /// Hand `armed` back to the firmware the way a session does once the
    /// channel it armed over has been retired: through whichever channel
    /// inventory publishes for the device now, which is this mouse's.
    pub(super) async fn hand_back(
        &self,
        armed: ArmedControls,
        armed_over: Arc<HidppChannel>,
    ) -> CaptureSessionOutcome {
        // The registry only has to resolve the route to a channel, so any
        // route will do.
        let route = DeviceRoute::Direct {
            vendor_id: crate::LOGITECH_VENDOR_ID,
            product_id: 0,
        };
        let retired = SharedChannel::new(armed_over, route.clone());
        let pending = armed
            .into_pending(&retired)
            .expect("the armed controls are owed back to the firmware");
        let registry = ChannelRegistry::default();
        let node = NodeId::from("fake-mouse".to_owned());
        registry.replace_node(node, [route], self.channel().await);
        pending.retry(&registry).await
    }

    /// Every mode a session set on the main wheel, in order.
    pub(super) fn main_wheel_mode_writes(&self) -> Vec<WheelMode> {
        self.lock()
            .main_wheel
            .as_ref()
            .map(|wheel| wheel.mode_writes.clone())
            .unwrap_or_default()
    }

    /// Every reporting a session set on the thumb wheel, in order.
    pub(super) fn thumbwheel_reporting_writes(&self) -> Vec<ThumbwheelReporting> {
        self.lock()
            .thumbwheel
            .as_ref()
            .map(|wheel| wheel.reporting_writes.clone())
            .unwrap_or_default()
    }

    fn lock(&self) -> MutexGuard<'_, Features> {
        self.features.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn reply(&self, request: &[u8]) -> Vec<u8> {
        let feature_index = request[FEATURE_INDEX];
        let function = request[FUNCTION_AND_SOFTWARE_ID] >> 4;
        let params = &request[PARAMS..];
        let answer = match feature_index {
            ROOT_FEATURE_INDEX => self.answer_root(function, params),
            MAIN_WHEEL_FEATURE_INDEX => self.answer_main_wheel(function, params),
            THUMBWHEEL_FEATURE_INDEX => self.answer_thumbwheel(function, params),
            _ => panic!("the fake mouse has no feature at index {feature_index}"),
        };
        // Every reply is a long report.
        let mut reply = vec![0; LONG_REPORT_LENGTH];
        reply[..PARAMS].copy_from_slice(&request[..PARAMS]);
        reply[REPORT_ID] = LONG_REPORT_ID;
        reply[PARAMS..PARAMS + answer.len()].copy_from_slice(&answer);
        reply
    }

    fn answer_root(&self, function: u8, params: &[u8]) -> Vec<u8> {
        match function {
            ROOT_GET_PROTOCOL_VERSION => vec![PROTOCOL_MAJOR_VERSION],
            ROOT_GET_FEATURE => {
                let feature = u16::from_be_bytes([params[0], params[1]]);
                vec![self.feature_index(feature)]
            }
            _ => panic!("the fake mouse has no root function {function}"),
        }
    }

    fn feature_index(&self, feature: u16) -> u8 {
        let features = self.lock();
        match feature {
            hires_wheel::FEATURE_ID if features.main_wheel.is_some() => MAIN_WHEEL_FEATURE_INDEX,
            thumbwheel::FEATURE_ID if features.thumbwheel.is_some() => THUMBWHEEL_FEATURE_INDEX,
            _ => ABSENT_FEATURE_INDEX,
        }
    }

    fn answer_main_wheel(&self, function: u8, params: &[u8]) -> Vec<u8> {
        let mut features = self.lock();
        let wheel = features
            .main_wheel
            .as_mut()
            .expect("only a main wheel is listed at this index");
        let answer = match function {
            hires_wheel::FN_GET_CAPABILITY => wheel.multiplier,
            hires_wheel::FN_GET_MODE => wheel.mode.to_byte(),
            hires_wheel::FN_SET_MODE => {
                let mode = WheelMode::from_byte(params[0]);
                wheel.mode = mode;
                wheel.mode_writes.push(mode);
                mode.to_byte()
            }
            _ => panic!("the fake main wheel has no function {function}"),
        };
        vec![answer]
    }

    fn answer_thumbwheel(&self, function: u8, params: &[u8]) -> Vec<u8> {
        let mut features = self.lock();
        let wheel = features
            .thumbwheel
            .as_mut()
            .expect("only a thumb wheel is listed at this index");
        match function {
            thumbwheel::FN_GET_INFO => wheel.info.to_payload().to_vec(),
            thumbwheel::FN_SET_REPORTING => {
                let reporting = ThumbwheelReporting::from_params(params);
                wheel.reporting_writes.push(reporting);
                Vec::new()
            }
            _ => panic!("the fake thumb wheel has no function {function}"),
        }
    }
}

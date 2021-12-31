use serde::Serialize;

use ross_protocol::event::bcm::BcmValue;
use ross_protocol::event::relay::{RelayDoubleExclusiveValue, RelayValue};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayState {
    pub device_states: Vec<DeviceState>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceState {
    pub peripheral_address: u16,
    pub peripheral_index: u8,
    pub peripheral_state: PeripheralState,
}

#[derive(Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PeripheralState {
    BcmSingle {
        #[serde(rename = "BRIGHTNESS")]
        brightness: u8,
    },
    BcmRgb {
        #[serde(rename = "RED")]
        red: u8,
        #[serde(rename = "GREEN")]
        green: u8,
        #[serde(rename = "BLUE")]
        blue: u8,
    },
    BcmRgbw {
        #[serde(rename = "RED")]
        red: u8,
        #[serde(rename = "GREEN")]
        green: u8,
        #[serde(rename = "BLUE")]
        blue: u8,
        #[serde(rename = "WHITE")]
        white: u8,
    },
    RelaySingle {
        #[serde(rename = "ON")]
        on: bool,
    },
    RelayDoubleExclusive {
        #[serde(rename = "FIRST")]
        first: bool,
        #[serde(rename = "SECOND")]
        second: bool,
    },
}

impl From<BcmValue> for PeripheralState {
    fn from(value: BcmValue) -> Self {
        match value {
            BcmValue::Single(brightness) => PeripheralState::BcmSingle { brightness },
            BcmValue::Rgb(red, green, blue) => PeripheralState::BcmRgb { red, green, blue },
            BcmValue::Rgbw(red, green, blue, white) => PeripheralState::BcmRgbw {
                red,
                green,
                blue,
                white,
            },
        }
    }
}

impl From<RelayValue> for PeripheralState {
    fn from(value: RelayValue) -> Self {
        match value {
            RelayValue::Single(on) => PeripheralState::RelaySingle { on },
            RelayValue::DoubleExclusive(value) => match value {
                RelayDoubleExclusiveValue::FirstChannelOn => {
                    PeripheralState::RelayDoubleExclusive {
                        first: true,
                        second: false,
                    }
                }
                RelayDoubleExclusiveValue::SecondChannelOn => {
                    PeripheralState::RelayDoubleExclusive {
                        first: false,
                        second: true,
                    }
                }
                RelayDoubleExclusiveValue::NoChannelOn => PeripheralState::RelayDoubleExclusive {
                    first: false,
                    second: false,
                },
            },
        }
    }
}

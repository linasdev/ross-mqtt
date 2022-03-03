use serde::Serialize;
use std::convert::TryFrom;

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
        #[serde(rename = "ON")]
        on: bool,
        #[serde(rename = "BRIGHTNESS")]
        brightness: u8,
    },
    BcmRgb {
        #[serde(rename = "ON")]
        on: bool,
        #[serde(rename = "RED")]
        red: u8,
        #[serde(rename = "GREEN")]
        green: u8,
        #[serde(rename = "BLUE")]
        blue: u8,
        #[serde(rename = "BRIGHTNESS")]
        brightness: u8,
    },
    BcmRgbw {
        #[serde(rename = "ON")]
        on: bool,
        #[serde(rename = "RED")]
        red: u8,
        #[serde(rename = "GREEN")]
        green: u8,
        #[serde(rename = "BLUE")]
        blue: u8,
        #[serde(rename = "WHITE")]
        white: u8,
        #[serde(rename = "BRIGHTNESS")]
        brightness: u8,
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

impl TryFrom<BcmValue> for PeripheralState {
    type Error = ();

    fn try_from(value: BcmValue) -> Result<Self, ()> {
        match value {
            BcmValue::Binary(_) => Err(()),
            BcmValue::Single(brightness) => Ok(PeripheralState::BcmSingle {
                on: brightness != 0,
                brightness,
            }),
            BcmValue::Rgb(red, green, blue, brightness) => Ok(PeripheralState::BcmRgb {
                on: (red != 0 || green != 0 || blue != 0) && brightness != 0,
                red,
                green,
                blue,
                brightness,
            }),
            BcmValue::Rgbw(red, green, blue, white, brightness) => Ok(PeripheralState::BcmRgbw {
                on: (red != 0 || green != 0 || blue != 0 || white != 0) && brightness != 0,
                red,
                green,
                blue,
                white,
                brightness,
            }),
        }
    }
}

impl TryFrom<RelayValue> for PeripheralState {
    type Error = ();

    fn try_from(value: RelayValue) -> Result<Self, ()> {
        match value {
            RelayValue::Single(on) => Ok(PeripheralState::RelaySingle { on }),
            RelayValue::DoubleExclusive(value) => match value {
                RelayDoubleExclusiveValue::FirstChannelOn => {
                    Ok(PeripheralState::RelayDoubleExclusive {
                        first: true,
                        second: false,
                    })
                }
                RelayDoubleExclusiveValue::SecondChannelOn => {
                    Ok(PeripheralState::RelayDoubleExclusive {
                        first: false,
                        second: true,
                    })
                }
                RelayDoubleExclusiveValue::NoChannelOn => {
                    Ok(PeripheralState::RelayDoubleExclusive {
                        first: false,
                        second: false,
                    })
                }
            },
        }
    }
}

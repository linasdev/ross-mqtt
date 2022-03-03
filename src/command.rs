use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayCommand {
    pub device_commands: Vec<DeviceCommand>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCommand {
    pub peripheral_address: u16,
    pub peripheral_index: u8,
    #[serde(flatten)]
    pub payload: CommandPayload,
}

#[derive(Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandPayload {
    RelayTurnOnSingle {},
    RelayTurnOffSingle {},
    BcmTurnOn {},
    BcmTurnOff {},
    BcmSetSingle {
        #[serde(rename = "BRIGHTNESS")]
        brightness: u8,
    },
    BcmSetRgb {
        #[serde(rename = "RED")]
        red: u8,
        #[serde(rename = "GREEN")]
        green: u8,
        #[serde(rename = "BLUE")]
        blue: u8,
    },
    BcmSetWhite {
        #[serde(rename = "BRIGHTNESS")]
        brightness: u8,
    },
}

use dotenv::dotenv;
use dotenv_codegen::dotenv;
use std::time::Duration;
use std::sync::{Arc, Mutex};

use ross_protocol::protocol::{Protocol, BROADCAST_ADDRESS};
use ross_protocol::interface::serial::Serial;
use ross_protocol::event::bcm::{BcmValue, BcmChangeBrightnessEvent};
use ross_protocol::event::relay::RelaySetValueEvent;
use ross_protocol::convert_packet::ConvertPacket;
use ross_configurator::get_programmer::get_programmer;

use crate::command::CommandPayload;
use crate::mqtt::{MqttCreateOptions, MqttConnectOptions, Mqtt};
use crate::state::{DeviceState, GatewayState, PeripheralState};

mod mqtt;
mod command;
mod state;

const TRANSACTION_RETRY_COUNT: u64 = 3;
const PACKET_TIMEOUT_MS: u64 = 50;
const BAUDRATE: u64 = 115200;

fn main() {
    dotenv().ok();

    let gcp_project_id = dotenv!("GCP_PROJECT_ID");
    let gcp_region = dotenv!("GCP_REGION");
    let gcp_registry_name = dotenv!("GCP_REGISTRY_NAME");
    
    let hostname = dotenv!("HOSTNAME");
    let gateway_id = dotenv!("GATEWAY_ID");
    let username = dotenv!("USERNAME");
    let trust_store_path = dotenv!("TRUST_STORE_PATH");
    let token_expiry_time_s = dotenv!("TOKEN_EXPIRY_TIME_S").parse::<i64>().unwrap();

    let device = dotenv!("DEVICE");

    let mut mqtt = Mqtt::new(MqttCreateOptions {
        gcp_project_id,
        gcp_region,
        gcp_registry_name,
        hostname,
        gateway_id,
    }).unwrap();

    mqtt.connect(MqttConnectOptions {
        trust_store_path,
        username,
        token_expiry_time_s,
    }).unwrap();

    mqtt.subscribe_to_commands(1).unwrap();

    let state_updates = Arc::new(Mutex::new(None));
    let commands = Arc::new(Mutex::new(vec![]));

    let state_updates_clone = Arc::clone(&state_updates);
    let commands_clone = Arc::clone(&commands);
    std::thread::spawn(move || {
        mqtt.start_loop(state_updates_clone, commands_clone);
    });

    let mut protocol = {
        let port = serialport::new(device, BAUDRATE as u32)
            .timeout(Duration::from_millis(
                TRANSACTION_RETRY_COUNT * PACKET_TIMEOUT_MS,
            ))
            .open().unwrap();

        let serial = Serial::new(port);
        Protocol::new(BROADCAST_ADDRESS, serial)
    };

    let programmer = get_programmer(&mut protocol).unwrap();

    protocol.add_packet_handler(Box::new(|packet, _protocol| {
        if let Ok(event) = BcmChangeBrightnessEvent::try_from_packet(packet) {
            *state_updates.lock().unwrap() = Some(GatewayState {
                device_states: vec![
                    DeviceState {
                        peripheral_address: event.transmitter_address,
                        peripheral_index: event.index,
                        peripheral_state: event.value.into(),
                    }
                ],
            });
        }

        if let Ok(event) = RelaySetValueEvent::try_from_packet(packet) {
            *state_updates.lock().unwrap() = Some(GatewayState {
                device_states: vec![
                    DeviceState {
                        peripheral_address: event.transmitter_address,
                        peripheral_index: event.index,
                        peripheral_state: event.value.into(),
                    }
                ],
            });
        }
    }), false).unwrap();

    loop {
        if let Err(err) = protocol.tick() {
            println!("Unexpected error occurred: {:?}", err);
        }

        if let Some(gateway_command) = commands.lock().unwrap().pop() {
            for device_command in gateway_command.device_commands.iter() {
                let packet = match device_command.payload {
                    CommandPayload::BcmSetSingle {
                        brightness
                    } => BcmChangeBrightnessEvent {
                        bcm_address: device_command.peripheral_address,
                        transmitter_address: programmer.programmer_address,
                        index: device_command.peripheral_index,
                        value: BcmValue::Single(brightness),
                    }.to_packet(),
                    CommandPayload::BcmSetRgb {
                        red,
                        green,
                        blue,
                    } => BcmChangeBrightnessEvent {
                        bcm_address: device_command.peripheral_address,
                        transmitter_address: programmer.programmer_address,
                        index: device_command.peripheral_index,
                        value: BcmValue::Rgb(red, green, blue),
                    }.to_packet(),
                    CommandPayload::BcmSetRgbw {
                        red,
                        green,
                        blue,
                        white,
                    } => BcmChangeBrightnessEvent {
                        bcm_address: device_command.peripheral_address,
                        transmitter_address: programmer.programmer_address,
                        index: device_command.peripheral_index,
                        value: BcmValue::Rgbw(red, green, blue, white),
                    }.to_packet(),
                };

                if let Err(err) = protocol.send_packet(&packet) {
                    println!("Failed to send packet with error ({:?})", err);
                } else {
                    println!("Sent packet ({:?})", packet);
                }
            }
        }
    }
}

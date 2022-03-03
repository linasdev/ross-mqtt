use dotenv::dotenv;
use dotenv_codegen::dotenv;
use std::convert::TryInto;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ross_configurator::get_devices::get_devices;
use ross_configurator::get_programmer::get_programmer;
use ross_protocol::convert_packet::ConvertPacket;
use ross_protocol::event::bcm::{BcmChangeBrightnessEvent, BcmValue};
use ross_protocol::event::gateway::GatewayDiscoverEvent;
use ross_protocol::event::relay::{RelaySetValueEvent, RelayValue};
use ross_protocol::interface::serial::Serial;
use ross_protocol::protocol::{Protocol, BROADCAST_ADDRESS};

use crate::command::CommandPayload;
use crate::mqtt::{Mqtt, MqttConnectOptions, MqttCreateOptions};
use crate::state::{DeviceState, GatewayState};

mod command;
mod mqtt;
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
    })
    .unwrap();

    mqtt.connect(MqttConnectOptions {
        trust_store_path,
        username,
        token_expiry_time_s,
    })
    .unwrap();

    mqtt.subscribe_to_commands(1).unwrap();

    let state_updates = Arc::new(Mutex::new(None));
    let commands = Arc::new(Mutex::new(vec![]));
    let discover = Arc::new(Mutex::new(false));

    let state_updates_clone = Arc::clone(&state_updates);
    let commands_clone = Arc::clone(&commands);
    let discover_clone = Arc::clone(&discover);

    std::thread::spawn(move || {
        mqtt.start_loop(state_updates_clone, commands_clone, discover_clone);
    });

    let mut protocol = {
        let port = serialport::new(device, BAUDRATE as u32)
            .timeout(Duration::from_millis(
                TRANSACTION_RETRY_COUNT * PACKET_TIMEOUT_MS,
            ))
            .open()
            .unwrap();

        let serial = Serial::new(port);
        Protocol::new(BROADCAST_ADDRESS, serial)
    };

    let programmer = get_programmer(&mut protocol).unwrap();

    protocol
        .add_packet_handler(
            Box::new(|packet, _protocol| {
                if let Ok(event) = BcmChangeBrightnessEvent::try_from_packet(packet) {
                    if let Ok(peripheral_state) = event.value.try_into() {
                        *state_updates.lock().unwrap() = Some(GatewayState {
                            device_states: vec![DeviceState {
                                peripheral_address: event.transmitter_address,
                                peripheral_index: event.index,
                                peripheral_state,
                            }],
                        });
                    }
                }

                if let Ok(event) = RelaySetValueEvent::try_from_packet(packet) {
                    if let Ok(peripheral_state) = event.value.try_into() {
                        *state_updates.lock().unwrap() = Some(GatewayState {
                            device_states: vec![DeviceState {
                                peripheral_address: event.transmitter_address,
                                peripheral_index: event.index,
                                peripheral_state,
                            }],
                        });
                    }
                }
            }),
            false,
        )
        .unwrap();

    loop {
        if let Err(err) = protocol.tick() {
            println!("Unexpected error occurred: {:?}", err);
        }

        if let Some(gateway_command) = commands.lock().unwrap().pop() {
            for device_command in gateway_command.device_commands.iter() {
                let packet = match device_command.payload {
                    CommandPayload::RelayTurnOnSingle {} => RelaySetValueEvent {
                        relay_address: device_command.peripheral_address,
                        transmitter_address: programmer.programmer_address,
                        index: device_command.peripheral_index,
                        value: RelayValue::Single(true),
                    }
                    .to_packet(),
                    CommandPayload::RelayTurnOffSingle {} => RelaySetValueEvent {
                        relay_address: device_command.peripheral_address,
                        transmitter_address: programmer.programmer_address,
                        index: device_command.peripheral_index,
                        value: RelayValue::Single(false),
                    }
                    .to_packet(),
                    CommandPayload::BcmTurnOn {} => BcmChangeBrightnessEvent {
                        bcm_address: device_command.peripheral_address,
                        transmitter_address: programmer.programmer_address,
                        index: device_command.peripheral_index,
                        value: BcmValue::Binary(true),
                    }
                    .to_packet(),
                    CommandPayload::BcmTurnOff {} => BcmChangeBrightnessEvent {
                        bcm_address: device_command.peripheral_address,
                        transmitter_address: programmer.programmer_address,
                        index: device_command.peripheral_index,
                        value: BcmValue::Binary(false),
                    }
                    .to_packet(),
                    CommandPayload::BcmSetSingle { brightness } => BcmChangeBrightnessEvent {
                        bcm_address: device_command.peripheral_address,
                        transmitter_address: programmer.programmer_address,
                        index: device_command.peripheral_index,
                        value: BcmValue::Single(brightness),
                    }
                    .to_packet(),
                    CommandPayload::BcmSetRgb { red, green, blue, brightness } => BcmChangeBrightnessEvent {
                        bcm_address: device_command.peripheral_address,
                        transmitter_address: programmer.programmer_address,
                        index: device_command.peripheral_index,
                        value: BcmValue::Rgb(red, green, blue, brightness),
                    }
                    .to_packet(),
                    CommandPayload::BcmSetRgbw { red, green, blue, white, brightness } => BcmChangeBrightnessEvent {
                        bcm_address: device_command.peripheral_address,
                        transmitter_address: programmer.programmer_address,
                        index: device_command.peripheral_index,
                        value: BcmValue::Rgbw(red, green, blue, white, brightness),
                    }
                    .to_packet(),
                };

                if let Err(err) = protocol.send_packet(&packet) {
                    println!("Failed to send packet with error ({:?})", err);
                } else {
                    println!("Sent packet ({:?})", packet);
                }
            }
        }

        if *discover.lock().unwrap() {
            let devices = get_devices(&mut protocol, &programmer).unwrap();

            for device in devices {
                let packet = GatewayDiscoverEvent {
                    device_address: device.bootloader_address,
                    gateway_address: programmer.programmer_address,
                }
                .to_packet();

                if let Err(err) = protocol.send_packet(&packet) {
                    println!("Failed to send packet with error ({:?})", err);
                } else {
                    println!("Sent packet ({:?})", packet);
                }
            }

            *discover.lock().unwrap() = false;
        }
    }
}

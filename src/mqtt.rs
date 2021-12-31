use serde::Serialize;
use paho_mqtt::{ Message, Client, CreateOptionsBuilder, SslOptionsBuilder, ConnectOptionsBuilder, SslVersion};
use jsonwebtoken::{encode, Header, EncodingKey, Algorithm};
use std::io::{Read, BufReader};
use std::fs::File;
use chrono::Utc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::command::GatewayCommand;
use crate::state::GatewayState;

#[derive(Debug, Serialize)]
struct JwtClaims<'a> {
    aud: &'a str,
    iat: i64,
    exp: i64,
}

pub struct MqttCreateOptions<'a> {
    pub gcp_project_id: &'a str,
    pub gcp_region: &'a str,
    pub gcp_registry_name: &'a str,
    pub hostname: &'a str,
    pub gateway_id: &'a str,
}

pub struct MqttConnectOptions<'a> {
    pub trust_store_path: &'a str,
    pub username: &'a str,
    pub token_expiry_time_s: i64,
}

pub struct Mqtt<'a> {
    client: Client,
    rx: Option<Receiver<Option<Message>>>,
    gcp_project_id: &'a str,
    gateway_id: &'a str,
}

impl<'a> Mqtt<'a> {
    pub fn new(create_options: MqttCreateOptions<'a>) -> Result<Self, paho_mqtt::Error> {
        let client_id = format!("projects/{}/locations/{}/registries/{}/devices/{}", create_options.gcp_project_id, create_options.gcp_region, create_options.gcp_registry_name, create_options.gateway_id);

        let create_opts = CreateOptionsBuilder::new()
            .client_id(client_id)
            .server_uri(create_options.hostname)
            .finalize();

        let client = Client::new(create_opts)?;

        Ok(Mqtt {
            client,
            rx: None,
            gcp_project_id: create_options.gcp_project_id,
            gateway_id: create_options.gateway_id,
        })
    }

    pub fn connect(&mut self, connect_options: MqttConnectOptions<'a>) -> Result<(), jsonwebtoken::errors::Error> {
        let ssl_opts = SslOptionsBuilder::new()
            .trust_store(connect_options.trust_store_path).unwrap()
            .ssl_version(SslVersion::Tls_1_2)
            .verify(true)
            .disable_default_trust_store(true)
            .enable_server_cert_auth(true)
            .finalize();

        let conn_opts = ConnectOptionsBuilder::new()
            .ssl_options(ssl_opts)
            .user_name(connect_options.username)
            .password(self.generate_password(connect_options.token_expiry_time_s)?)
            .finalize();

        self.client.connect(conn_opts).unwrap();

        self.rx = Some(self.client.start_consuming());

        Ok(())
    }

    pub fn subscribe_to_commands(&mut self, qos: i32) -> Result<(), paho_mqtt::Error> {
        let command_topic = format!("/devices/{}/commands/#", self.gateway_id);
        self.client.subscribe(&command_topic, qos)?;
        Ok(())
    }

    pub fn start_loop(self, state_updates: Arc<Mutex<Option<GatewayState>>>, commands: Arc<Mutex<Vec<GatewayCommand>>>) {
        loop {
            let mut state_update = state_updates.lock().unwrap();

            match self.rx.as_ref().unwrap().try_recv() {
                Ok(msg) => {
                    if let Some(msg) = msg {
                        match serde_json::de::from_str(&*msg.payload_str()) {
                            Ok(gateway_command) => {
                                commands.lock().unwrap().push(gateway_command);
                            },
                            Err(err) => {
                                println!("Gateway command deserialization failed with error: {}", err);
                            }
                        }
                    } else {
                    }
                }
                Err(TryRecvError::Empty) => {},
                Err(TryRecvError::Disconnected) => break,
            }

            if let Some(gateway_state) = &*state_update {
                let state_topic = format!("/devices/{}/state", self.gateway_id);

                let content = serde_json::ser::to_string(&gateway_state).unwrap();
                let message = Message::new(&state_topic, content, 1);
                
                self.client.publish(message).unwrap();

                *state_update = None;
                drop(state_update);

                thread::sleep(Duration::from_secs(1));
            }
        }
    }

    fn generate_password(&self, token_expiry_time_s: i64) -> Result<String, jsonwebtoken::errors::Error> {
        let now = Utc::now().timestamp();

        let my_claims = JwtClaims {
            aud: self.gcp_project_id,
            iat: now,
            exp: now + token_expiry_time_s,
        };
        
        let f = File::open("./rsa_private.pem").unwrap();
        let mut reader = BufReader::new(f);
        let mut private_key = Vec::new();
        reader.read_to_end(&mut private_key).unwrap();

        encode(
            &Header::new(Algorithm::RS256),
            &my_claims,
            &EncodingKey::from_rsa_pem(&private_key).unwrap()
        )
    }
}

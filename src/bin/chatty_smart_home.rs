use std::path::PathBuf;

use anyhow::Context;
use async_openai::{types::CreateTranscriptionRequestArgs, Client};
use base64::{engine::general_purpose, Engine};
use chatty::{
    chat_manager::{self, MqttChatStreamDisplay},
    configuration::AppConfig,
    mqtt::start_mqtt_service_with_subs,
    utils::{now_rfc3339, QUESTION_MARK_EMOJI, ROBOT_EMOJI, VOICE_TO_TEXT_TRANSCRIBE_MODEL},
};
use clap::Parser;
use dialoguer::console::{style, Term};
use rumqttc::QoS;
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use tempdir::TempDir;

const SMART_HOME_MQTT_TOPIC: &str = "chatty/home_state/simple/v2";
const SMART_HOME_VOICE_COMMAND: &str = "chatty/audio_command/simple";
const SMART_HOME_RESET_CHAT_MANAGER_COMMAND: &str = "chatty/audio_command/reset_chat_manager";
const SMART_HOME_TEXT_OUTPUT_TOPIC: &str = "chatty/audio_command/response/transcript";

#[derive(Parser, Debug)]
#[command()]
struct Cli {
    /// config path
    #[arg(long)]
    config: Option<PathBuf>,
    /// disable streaming
    #[arg(long)]
    disable_streaming: bool,
    /// do not save conversation
    #[arg(long)]
    no_save: bool,
    /// save default config and exit
    #[arg(long)]
    create_config: bool,
    /// copy token from local config to user config
    #[arg(long)]
    copy_local_config: bool,
    /// Do not speak response
    #[arg(short, long)]
    mute: bool,

    /// The audio device to use
    #[arg(short, long)]
    device: Option<String>,
    /// Use the JACK host
    #[arg(short, long)]
    jack: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.create_config {
        // this is a meh way to do this
        let config_new = AppConfig {
            open_ai_api_key: String::from("EMPTY_TOKEN"),
            mqtt: None,
        };
        config_new.save_user_config()?;
        return Ok(());
    }

    if cli.copy_local_config {
        // this is a meh way to do this
        let local_config = AppConfig::load_dev_config()?;
        local_config.save_user_config()?;
        return Ok(());
    }

    let config = if let Some(config_path) = &cli.config {
        AppConfig::load_config(config_path)?
    } else {
        AppConfig::load_user_config()?
    };

    let client = Client::new().with_api_key(&config.open_ai_api_key);

    let mut mqtt_config = config.mqtt.context("mqtt config missing")?.clone();
    mqtt_config.client_id = String::from("smart_home_mqtt_server");

    let (mqtt_client, mut message_receiver) = start_mqtt_service_with_subs(
        &mqtt_config,
        vec![
            String::from(SMART_HOME_MQTT_TOPIC),
            String::from(SMART_HOME_RESET_CHAT_MANAGER_COMMAND),
            String::from(SMART_HOME_VOICE_COMMAND),
        ],
    )
    .await?;

    let smart_home_state_schema = schema_for!(SmartHomeState);
    let smart_home_state_schema_json = serde_json::to_string_pretty(&smart_home_state_schema)?;

    let system_messages = format!(
        "You are an AI in charge of a smart home. Each message will start with
json of the current home status followed by a user request.
Respond with json of the updated smart home state followed by a message for the user.
Schema for smart home state is {smart_home_state_schema_json}.
Message for user should be prefaced with a line that says \"MESSAGE:\""
    );

    let mut chat_manager = chat_manager::ChatHistory::new(&system_messages)?;

    let term = Term::stdout();

    term.write_line(&system_messages)?;

    let mut smart_home_state = SmartHomeState::default();

    while let Some(message) = message_receiver.recv().await {
        match message.topic.as_ref() {
            SMART_HOME_MQTT_TOPIC => {
                smart_home_state = SmartHomeState::from_json_slice(&message.payload)?;
            }
            SMART_HOME_RESET_CHAT_MANAGER_COMMAND => {
                term.write_line("Resetting chat manager")?;
                chat_manager = chat_manager::ChatHistory::new(&system_messages)?;
            }
            SMART_HOME_VOICE_COMMAND => {
                let message: AudioMessage =
                    serde_json::from_slice(&message.payload).context("failed to parse json")?;
                let temp_dir = TempDir::new("audio_message_temp_dir")?;
                let temp_auido_file = temp_dir.path().join(format!("recorded.{}", message.format));
                let decoded_file = general_purpose::STANDARD
                    .decode(&message.data)
                    .context("Failed to parse base64")?;
                std::fs::write(&temp_auido_file, &decoded_file)?;
                term.write_line("Transcribing\n")?;

                let request = CreateTranscriptionRequestArgs::default()
                    .file(temp_auido_file)
                    .model(VOICE_TO_TEXT_TRANSCRIBE_MODEL)
                    .build()?;

                let response = client.audio().transcribe(request).await?;
                let user_question = response.text;

                let smart_home_state_json = smart_home_state.to_json()?;

                let current_date_time = now_rfc3339();
                let message = format!(
                    "CURRENT_DATE_TIME: {current_date_time}\nHOUSE_STATE:\n```json\n{smart_home_state_json}\n```\nUSER_REQUEST:\n{user_question}"
                );

                term.write_line(&format!("{QUESTION_MARK_EMOJI} Question:\n{message}"))?;

                term.write_line(&format!("\n{ROBOT_EMOJI} ChatGPT:\n"))?;

                let response = if cli.disable_streaming {
                    let response = chat_manager.next_message(&message, &client).await?;
                    term.write_line(&response)?;
                    term.write_line("")?;
                    response
                } else {
                    let mut mqtt_streamer = MqttChatStreamDisplay::new(
                        SMART_HOME_TEXT_OUTPUT_TOPIC,
                        mqtt_client.clone(),
                    );
                    chat_manager
                        .next_message_stream_stdout(
                            &message,
                            &client,
                            &term,
                            Some(&mut mqtt_streamer),
                        )
                        .await?
                };

                match extract_json(&response) {
                    Ok(Some(message)) => {
                        smart_home_state = message;
                        term.write_line(&format!(
                            "{}",
                            style(smart_home_state.to_json()?).green()
                        ))?;

                        mqtt_client
                            .publish(
                                SMART_HOME_MQTT_TOPIC,
                                QoS::AtMostOnce,
                                true,
                                smart_home_state.to_json()?,
                            )
                            .await?;
                    }
                    Err(error) => {
                        term.write_line(&format!("Failed to parse json {:?}", error))?;
                    }
                    _ => (),
                }

                let user_message_start = response.find("MESSAGE:");

                if let Some(user_message) = response.get(user_message_start.unwrap_or(0)..) {
                    let user_message_trimmed =
                        user_message.replace("MESSAGE:", "").trim().to_owned();
                    if !cli.mute {
                        mqtt_client
                            .publish(
                                "home_speak/say/cheerful",
                                QoS::AtMostOnce,
                                false,
                                user_message_trimmed,
                            )
                            .await?;
                    }
                }

                if !cli.no_save {
                    chat_manager.save_to_file()?;
                }
            }
            _ => (),
        }
    }
    Ok(())
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct AudioMessage {
    pub data: String,
    pub format: String,
}

fn extract_json(message: &str) -> anyhow::Result<Option<SmartHomeState>> {
    let json_range = message
        .find('{')
        .and_then(|start| message.rfind('}').map(|end| (start, end + 1)));

    if let Some((json_start, json_end)) = json_range {
        if let Some(json) = message.get(json_start..json_end) {
            match SmartHomeState::from_json(json) {
                Ok(parsed_state) => return Ok(Some(parsed_state)),
                Err(error) => {
                    // you could try again by showing ChatGPT the error :D
                    return Err(anyhow::anyhow!("Failed to parse json {:?}", error));
                }
            }
        }
    }
    Ok(None)
}

#[derive(Deserialize, Serialize, JsonSchema, Debug, Clone, Default)]
pub struct SmartHomeState {
    pub lights: HomeLightsState,
}

impl SmartHomeState {
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn from_json_slice(data: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(data)?)
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

#[derive(Deserialize, Serialize, JsonSchema, Debug, Clone, Default)]
pub struct HomeLightsState {
    #[serde(default)]
    bedroom: Light,
    #[serde(default)]
    living_room: Light,
    #[serde(default)]
    hallway: Light,
    #[serde(default)]
    living_room_mood_lights: Light,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug, Clone, Default)]
pub struct Light {
    state: LightState,
    #[validate(range(min = 0, max = 255))]
    brightness: u8,
    color: ColorMode,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug, Clone, Copy, Default)]
pub enum LightState {
    On,
    #[default]
    Off,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug, Clone)]
pub enum ColorMode {
    Temperature { color_temperature: ColorTemperature },
    Color { hex_color: String },
}

impl Default for ColorMode {
    fn default() -> Self {
        Self::Temperature {
            color_temperature: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum ColorTemperature {
    Coolest,
    Cool,
    #[default]
    Neutral,
    Warm,
    Warmest,
}

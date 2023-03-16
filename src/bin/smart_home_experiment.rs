use anyhow::Context;
use async_openai::{types::CreateTranscriptionRequestArgs, Client};
use chatty::{
    chat_manager::{self},
    configuration::AppConfig,
    mqtt::start_mqtt_service_with_subs,
    utils::{now_rfc3339, QUESTION_MARK_EMOJI, ROBOT_EMOJI, VOICE_TO_TEXT_TRANSCRIBE_MODEL},
};
use clap::Parser;
use dialoguer::console::{style, Term};
use rumqttc::{Publish, QoS};
use schemars::{schema_for, JsonSchema};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::BufRead;
use tokio::sync::mpsc::Receiver;

const SMART_HOME_MQTT_TOPIC: &str = "chatty/home_state/simple";

#[derive(Parser, Debug)]
#[command()]
struct Cli {
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

    let config = AppConfig::load_user_config()?;

    let client = Client::new().with_api_key(&config.open_ai_api_key);

    let (mqtt_client, mut message_receiver) = start_mqtt_service_with_subs(
        &config.mqtt.context("mqtt config missing")?,
        vec![String::from(SMART_HOME_MQTT_TOPIC)],
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

    //     let system_messages = "You are an AI in charge of a smart home. Each message will start with
    // json of the current home status followed by a user request.
    // Respond with json of the updated smart home state followed by a message for the user.
    // Message for user should be prefaced with a line that says \"MESSAGE:\""
    //         .to_owned();

    let mut chat_manager = chat_manager::ChatHistory::new(&system_messages)?;

    let term = Term::stdout();

    term.write_line(&system_messages)?;

    let mut smart_home_state =
        wait_for_first_mqtt_message(&mut message_receiver, SMART_HOME_MQTT_TOPIC).await?;

    loop {
        let (_temp_dir, audio_path) =
            chatty::audio::record_audio_with_cli(cli.jack, cli.device.clone())?;

        term.write_line("Transcribing\n")?;

        let request = CreateTranscriptionRequestArgs::default()
            .file(audio_path)
            .model(VOICE_TO_TEXT_TRANSCRIBE_MODEL)
            .build()?;

        let response = client.audio().transcribe(request).await?;
        let user_question = response.text;

        // make sure we are not reading outdated info
        while let Ok(message) = message_receiver.try_recv() {
            if message.topic == *SMART_HOME_MQTT_TOPIC {
                smart_home_state = SmartHomeState::from_json_slice(&message.payload)?;
            }
        }

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
            chat_manager
                .next_message_stream_stdout(&message, &client, &term, None)
                .await?
        };

        let json_range = response
            .find('{')
            .and_then(|start| response.rfind('}').map(|end| (start, end + 1)));

        if let Some((json_start, json_end)) = json_range {
            if let Some(json) = response.get(json_start..json_end) {
                match SmartHomeState::from_json(json) {
                    Ok(parsed_state) => {
                        smart_home_state = parsed_state;
                        term.write_line(&format!("{}", style(smart_home_state.to_json()?).red()))?;

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
                        // you could try again by showing ChatGPT the error :D
                        term.write_line(&format!("Failed to parse json {:?}", error))?;
                    }
                }
            }
        }

        let user_message_start = response.find("MESSAGE:");

        if let Some(user_message) = response.get(user_message_start.unwrap_or(0)..) {
            let user_message_trimmed = user_message.replace("MESSAGE:", "").trim().to_owned();
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

        wait_for_enter()?;
    }
}

fn wait_for_enter() -> anyhow::Result<()> {
    println!("Press enter to continue recording");
    std::io::stdin().lock().read_line(&mut String::new())?;
    Ok(())
}

#[derive(Deserialize, Serialize, JsonSchema, Debug, Clone)]
pub struct SmartHomeState {
    pub lights: HomeLightsState,
    #[serde(default)]
    pub alarms: Vec<Alarm>,
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

#[derive(Deserialize, Serialize, JsonSchema, Debug, Clone, Copy)]
pub struct HomeLightsState {
    #[serde(default)]
    bedroom: LightState,
    #[serde(default)]
    living_room: LightState,
    #[serde(default)]
    hallway: LightState,
    #[serde(default)]
    living_room_mood_lights: LightState,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug, Clone, Copy, Default)]
pub enum LightState {
    On,
    #[default]
    Off,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug, Clone)]
pub struct Alarm {
    pub iso8601_time: String,
    pub active: bool,
    pub name: String,
}

async fn wait_for_first_mqtt_message<T>(
    message_receiver: &mut Receiver<Publish>,
    topic: &str,
) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    loop {
        // this is an odd way to do it
        while let Some(message) = message_receiver.recv().await {
            if message.topic == *topic {
                return Ok(serde_json::from_slice::<T>(&message.payload)?);
            }
        }
    }
}

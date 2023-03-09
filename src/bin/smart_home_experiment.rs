use anyhow::Context;
use async_openai::{types::CreateTranscriptionRequestArgs, Client};
use chatty::{
    chat_manager::{self},
    configuration::AppConfig,
    mqtt::start_mqtt_service_with_subs,
};
use clap::Parser;
use dialoguer::console::{style, Emoji, Term};
use rumqttc::QoS;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::BufRead};

const ROBOT_EMOJI: Emoji = Emoji("🤖", "ChatGPT");
const QUESTION_MARK_EMOJI: Emoji = Emoji("❓", "ChatGPT");

const TRANSCRIBE_MODEL: &str = "whisper-1";

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
        vec![String::from("chatty/home_state/simple")],
    )
    .await?;

    let system_messages = format!(
        "You are an AI in charge of a smart home. Each message will start with 
json of the current home status followed by a user request.
Respond with json of the updated smart home state followed by a message for the user.
Message for user should be prefaced with a line that says \"MESSAGE:\""
    );

    let mut chat_manager = chat_manager::ChatHistory::new(&system_messages)?;

    let term = Term::stdout();

    let mut smart_home_state = SmartHomeState::default();

    // this is an odd way to do it :D
    if let Ok(message) = message_receiver.try_recv() {
        if message.topic == String::from("chatty/home_state/simple") {
            smart_home_state = SmartHomeState::from_json_slice(&message.payload)?;
        }
    }

    loop {
        let (_temp_dir, audio_path) =
            chatty::audio::record_audio_with_cli(cli.jack, cli.device.clone())?;

        term.write_line("Transcribing\n")?;

        let request = CreateTranscriptionRequestArgs::default()
            .file(audio_path)
            .model(TRANSCRIBE_MODEL)
            .build()?;

        let response = client.audio().transcribe(request).await?;
        let user_question = response.text;

        let smart_home_state_json = smart_home_state.to_json()?;

        let message = format!(
            "HOUSE_STATE:\n```json\n{smart_home_state_json}\n```\nUSER_REQUEST:\n{user_question}"
        );

        term.write_line(&format!("{QUESTION_MARK_EMOJI} Question:\n{message}"))?;

        term.write_line(&format!("\n{ROBOT_EMOJI} ChatGPT:\n"))?;

        let response = if cli.disable_streaming {
            let response = chat_manager.next_message(&message, &client).await?;
            term.write_line(&response)?;
            term.write_line("")?;
            response
        } else {
            let response = chat_manager
                .next_message_stream_stdout(&message, &client, &term)
                .await?;
            response
        };

        let json_range = response
            .find("{")
            .map(|start| response.rfind("}").map(|end| (start, end + 1)))
            .flatten();

        if let Some((json_start, json_end)) = json_range {
            if let Some(json) = response.get(json_start..json_end) {
                smart_home_state = SmartHomeState::from_json(json)?;

                term.write_line(&format!("{}", style(smart_home_state.to_json()?).red()))?;

                mqtt_client
                    .publish(
                        "chatty/home_state/simple",
                        QoS::AtMostOnce,
                        true,
                        smart_home_state.to_json()?,
                    )
                    .await?;
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

        while let Ok(message) = message_receiver.try_recv() {
            if message.topic == String::from("chatty/home_state/simple") {
                smart_home_state = SmartHomeState::from_json_slice(&message.payload)?;
            }
        }

        wait_for_enter()?;
    }
}

fn wait_for_enter() -> anyhow::Result<()> {
    println!("Press enter to continue recording");
    std::io::stdin().lock().read_line(&mut String::new())?;
    Ok(())
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SmartHomeState {
    pub lights: HashMap<String, LightState>,
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

impl Default for SmartHomeState {
    fn default() -> Self {
        let mut lights = HashMap::new();
        lights.insert(String::from("bedroom"), LightState::Off);
        lights.insert(String::from("living_room"), LightState::Off);
        lights.insert(String::from("hallway"), LightState::Off);
        Self { lights }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub enum LightState {
    On,
    Off,
}

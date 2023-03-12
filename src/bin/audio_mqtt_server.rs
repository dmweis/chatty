use anyhow::{Context, Result};
use async_openai::types::CreateTranscriptionRequestArgs;
use async_openai::Client;
use base64::{engine::general_purpose, Engine as _};
use chatty::utils::VOICE_TO_TEXT_TRANSCRIBE_MODEL;
use chatty::{configuration::AppConfig, mqtt::start_mqtt_service_with_subs};
use clap::Parser;
use tempdir::TempDir;

use serde::{Deserialize, Serialize};

const AUDIO_MQTT_TOPIC: &str = "chatty/audio_command/simple";

#[derive(Parser, Debug)]
#[command()]
struct Cli {}

#[tokio::main]
async fn main() -> Result<()> {
    let _cli = Cli::parse();

    let config = AppConfig::load_dev_config()?;

    let client = Client::new().with_api_key(&config.open_ai_api_key);

    let mut mqtt_config = config.mqtt.context("mqtt config missing")?.clone();
    mqtt_config.client_id = String::from("Server");

    let (_mqtt_client, mut message_receiver) =
        start_mqtt_service_with_subs(&mqtt_config, vec![String::from(AUDIO_MQTT_TOPIC)]).await?;

    while let Some(message) = message_receiver.recv().await {
        if message.topic == *AUDIO_MQTT_TOPIC {
            let message: AudioMessage =
                serde_json::from_slice(&message.payload).context("failed to parse json")?;
            let temp_dir = TempDir::new("audio_message_temp_dir")?;
            let temp_auido_file = temp_dir.path().join(format!("recorded.{}", message.format));
            let decoded_file = general_purpose::STANDARD
                .decode(&message.data)
                .context("Failed to parse base64")?;
            std::fs::write(&temp_auido_file, &decoded_file)?;

            let request = CreateTranscriptionRequestArgs::default()
                .file(temp_auido_file)
                .model(VOICE_TO_TEXT_TRANSCRIBE_MODEL)
                .build()?;
            let response = client.audio().transcribe(request).await?;
            println!("Transcribed:\n{}", response.text);
        }
    }

    Ok(())
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct AudioMessage {
    pub data: String,
    pub format: String,
}

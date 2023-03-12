use anyhow::{Context, Result};
use chatty::configuration::AppConfig;
use chatty::mqtt::start_mqtt_service;
use clap::Parser;
use rumqttc::{self, QoS};

// heavily inspired by cpal record_wav example
// https://github.com/RustAudio/cpal/blob/master/examples/record_wav.rs

#[derive(Parser, Debug)]
#[command()]
struct Cli {
    /// The audio device to use
    #[arg(short, long)]
    device: Option<String>,

    /// Use the JACK host
    #[arg(short, long)]
    jack: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = AppConfig::load_dev_config()?;

    let mqtt_client = start_mqtt_service(&config.mqtt.context("mqtt config missing")?)?;

    let audio = chatty::audio::record_audio_with_cli_to_memory(cli.jack, cli.device)?;

    mqtt_client
        .publish(
            "chatty/audio_command/simple",
            QoS::AtMostOnce,
            false,
            create_message(&audio),
        )
        .await?;

    // wait so the message is sent...
    // not an ideal way to do this...
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    Ok(())
}
use base64::{engine::general_purpose, Engine as _};

use serde_json::json;

fn create_message(data: &[u8]) -> String {
    let base64_wav_file: String = general_purpose::STANDARD.encode(data);
    let data = json!({
        "data": base64_wav_file,
        "format": "wav"
    });
    data.to_string()
}

use async_openai::{types::CreateTranscriptionRequestArgs, Client};
use chatty::{chat_manager, configuration::get_configuration, mqtt::start_mqtt_service};
use clap::Parser;
use rumqttc::QoS;
use std::io::BufRead;

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
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = get_configuration()?;
    let client = Client::new().with_api_key(&config.open_ai_api_key);

    let mqtt_client = start_mqtt_service(&config.mqtt)?;

    let mut chat_manager =
        chat_manager::ChatHistory::new("You are Joi. The cheerful helpful AI assistant.")?;

    loop {
        let (_temp_dir, audio_path) =
            chatty::audio::record_audio_with_cli(cli.jack, cli.device.clone())?;

        println!("Transcribing");

        let request = CreateTranscriptionRequestArgs::default()
            .file(audio_path)
            .model("whisper-1")
            .build()?;

        let response = client.audio().transcribe(request).await?;
        let user_question = response.text;

        println!("User:\n\n{}", user_question);

        let response = chat_manager.next_message(&user_question, &client).await?;

        println!("ChatGPT:\n\n{}", response);

        mqtt_client
            .publish("home_speak/say/cheerful", QoS::AtMostOnce, false, response)
            .await?;

        wait_for_enter()?;
    }
}

fn wait_for_enter() -> anyhow::Result<()> {
    println!("Press enter to continue recording");
    std::io::stdin().lock().read_line(&mut String::new())?;
    Ok(())
}

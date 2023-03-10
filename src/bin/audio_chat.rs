use anyhow::Context;
use async_openai::{types::CreateTranscriptionRequestArgs, Client};
use chatty::{
    chat_manager::{self, generate_system_instructions},
    configuration::AppConfig,
    mqtt::start_mqtt_service,
    utils::{QUESTION_MARK_EMOJI, ROBOT_EMOJI, VOICE_TO_TEXT_TRANSCRIBE_MODEL},
};
use clap::Parser;
use dialoguer::console::Term;
use rumqttc::QoS;

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

    let mqtt_client = start_mqtt_service(&config.mqtt.context("mqtt config missing")?)?;

    let system_messages = generate_system_instructions();

    let mut chat_manager = chat_manager::ChatHistory::new(&system_messages["joi"])?;

    let term = Term::stdout();

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

        term.write_line(&format!("{QUESTION_MARK_EMOJI} Question:\n{user_question}"))?;

        term.write_line(&format!("\n{ROBOT_EMOJI} ChatGPT:\n"))?;

        let response = if cli.disable_streaming {
            let response = chat_manager.next_message(&user_question, &client).await?;
            term.write_line(&response)?;
            term.write_line("")?;
            response
        } else {
            chat_manager
                .next_message_stream_stdout(&user_question, &client, &term)
                .await?
        };

        if !cli.mute {
            mqtt_client
                .publish("home_speak/say/cheerful", QoS::AtMostOnce, false, response)
                .await?;
        }

        if !cli.no_save {
            chat_manager.save_to_file()?;
        }

        term.write_line("Press enter to continue recording")?;
        term.read_line()?;
    }
}

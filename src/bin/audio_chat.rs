use async_openai::{types::CreateTranscriptionRequestArgs, Client};
use chatty::{chat_manager, configuration::get_configuration};
use clap::Parser;

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

        let response = chat_manager.next_message(&response.text, &client).await?;

        println!("Query:\n{}", response);
    }
}

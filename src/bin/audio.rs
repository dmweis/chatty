use async_openai::{
    types::{
        ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs,
        CreateTranscriptionRequestArgs, Role,
    },
    Client,
};
use chatty::configuration::AppConfig;
use clap::Parser;

use futures::StreamExt;

#[derive(Parser, Debug)]
#[command()]
struct Cli {
    /// The audio device to use
    #[arg(short, long)]
    device: Option<String>,

    /// Use the JACK host
    #[arg(short, long)]
    jack: bool,

    /// use streaming
    #[arg(short, long)]
    stream: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let config = AppConfig::load_dev_config()?;
    let client = Client::new().with_api_key(&config.open_ai_api_key);

    let (_temp_dir, audio_path) = chatty::audio::record_audio_with_cli(cli.jack, cli.device)?;

    println!("Recording stopped. Transcribing");

    let request = CreateTranscriptionRequestArgs::default()
        .file(audio_path)
        .model("whisper-1")
        .build()?;

    let response = client.audio().transcribe(request).await?;

    println!("Query:\n{}", response.text);

    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
        .messages([ChatCompletionRequestMessageArgs::default()
            .content(response.text)
            .role(Role::User)
            .build()?])
        .build()?;

    if cli.stream {
        let mut stream = client.chat().create_stream(request).await?;

        // For reasons not documented in OpenAI docs / OpenAPI spec, the response of streaming call is different and doesn't include all the same fields.
        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    response.choices.iter().for_each(|chat_choice| {
                        if let Some(ref content) = chat_choice.delta.content {
                            print!("{}", content);
                        }
                    });
                }
                Err(err) => {
                    println!("error: {err}");
                }
            }
        }
    } else {
        let response = client.chat().create(request).await?;

        println!("\nResponse:\n");
        for choice in response.choices {
            println!(
                "{}: Role: {}  Content: {}",
                choice.index, choice.message.role, choice.message.content
            );
        }
    }

    Ok(())
}

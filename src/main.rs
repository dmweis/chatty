#![allow(dead_code)]

mod configuration;

use async_openai::{
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role},
    Client,
};
use clap::Parser;
use configuration::get_configuration;
use futures::StreamExt;

#[derive(Parser)]
#[command()]
struct Cli {
    /// use streaming
    #[arg(short, long)]
    stream: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = get_configuration()?;

    let client = Client::new().with_api_key(&config.open_ai_api_key);

    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
        .messages([ChatCompletionRequestMessageArgs::default()
            .content("write a song if Coldplay and AR Rahman collaborated together")
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

use async_openai::Client;
use chatty::{
    chat_manager::{self, generate_system_instructions},
    configuration::{read_user_config_file, save_user_config_file, ChattyCliConfig},
};
use clap::Parser;
use dialoguer::console::{Emoji, Term};

const ROBOT_EMOJI: Emoji = Emoji("🤖", "ChatGPT");
const QUESTION_MARK_EMOJI: Emoji = Emoji("❓", "ChatGPT");

#[derive(Parser)]
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.create_config {
        // this is a meh way to do this
        let config_new = ChattyCliConfig {
            open_ai_api_key: String::from("EMPTY_TOKEN"),
        };
        save_user_config_file(config_new)?;
        return Ok(());
    }

    let config = read_user_config_file()?;

    let client = Client::new().with_api_key(&config.open_ai_api_key);

    let system_messages = generate_system_instructions();

    let mut chat_manager = chat_manager::ChatHistory::new(&system_messages["joi"])?;

    let term = Term::stdout();

    loop {
        term.write_line(&format!("{QUESTION_MARK_EMOJI} Question:\n"))?;
        let user_question = term.read_line()?;

        term.write_line(&format!("\n{ROBOT_EMOJI} ChatGPT:\n"))?;

        if cli.disable_streaming {
            let response = chat_manager.next_message(&user_question, &client).await?;
            term.write_line(&response)?;
            term.write_line("")?;
        } else {
            let _response = chat_manager
                .next_message_stream_stdout(&user_question, &client, &term)
                .await?;
        }
        if !cli.no_save {
            chat_manager.save_to_file()?;
        }
    }
}

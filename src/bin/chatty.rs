use async_openai::Client;
use chatty::{
    chat_manager::{self, generate_system_instructions},
    configuration::AppConfig,
    utils::{CHAT_GPT_MODEL_TOKEN_LIMIT, QUESTION_MARK_EMOJI, ROBOT_EMOJI},
};
use clap::Parser;
use dialoguer::console::Term;

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
    /// copy token from local config to user config
    #[arg(long)]
    copy_local_config: bool,
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
        let config_new = AppConfig {
            open_ai_api_key: local_config.open_ai_api_key,
            mqtt: None,
        };
        config_new.save_user_config()?;
        return Ok(());
    }

    let config = AppConfig::load_user_config()?;

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

            // print usage
            if let Some(token_usage) = chat_manager.token_usage() {
                let total_token_usage = token_usage.total_tokens;
                term.write_line(&format!(
                    "\n{total_token_usage}/{CHAT_GPT_MODEL_TOKEN_LIMIT} tokens used"
                ))?;
            }

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

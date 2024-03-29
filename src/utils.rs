use anyhow::Context;
use chrono::{DateTime, Local};
use dialoguer::console::Emoji;
use std::{collections::HashMap, io::BufRead};
use tracing::dispatcher::{self, Dispatch};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Registry};

pub const CHAT_GPT_MODEL_NAME: &str = "gpt-3.5-turbo";
pub const CHAT_GPT_KNOWLEDGE_CUTOFF: &str = "September 2021";
pub const CHAT_GPT_MODEL_TOKEN_LIMIT: u32 = 4096;

pub const VOICE_TO_TEXT_TRANSCRIBE_MODEL: &str = "whisper-1";
pub const VOICE_TO_TEXT_TRANSCRIBE_MODEL_ENGLISH_LANGUAGE: &str = "en";

// Emojis
pub const ROBOT_EMOJI: Emoji = Emoji("🤖", "");
pub const QUESTION_MARK_EMOJI: Emoji = Emoji("❓", "");
pub const SYSTEM_EMOJI: Emoji = Emoji("ℹ️ ", "");
pub const INCREASING_TREND_EMOJI: Emoji = Emoji("📈", "");

pub fn now() -> DateTime<Local> {
    Local::now()
}

pub fn now_rfc3339() -> String {
    now().to_rfc3339()
}

pub fn wait_for_enter(message: &str) -> anyhow::Result<()> {
    // make make this not do new line?
    // but remember to flush
    println!("{}", message);
    std::io::stdin().lock().read_line(&mut String::new())?;
    Ok(())
}

pub const DEFAULT_SYSTEM_INSTRUCTIONS_KEY: &str = "default";

pub fn generate_system_instructions() -> HashMap<&'static str, String> {
    let mut instructions = HashMap::new();

    let current_time_str = now_rfc3339();

    instructions.insert(
        DEFAULT_SYSTEM_INSTRUCTIONS_KEY,
        format!(
            "You are ChatGPT, a large language model trained by OpenAI. 
Answer as concisely as possible. Knowledge cutoff year {} Current date and time: {}",
            CHAT_GPT_KNOWLEDGE_CUTOFF, current_time_str
        ),
    );

    instructions.insert(
        "joi",
        format!(
            "You are Joi. The cheerful and helpful AI assistant. 
Knowledge cutoff year {} Current date and time: {}",
            CHAT_GPT_KNOWLEDGE_CUTOFF, current_time_str
        ),
    );

    instructions
}

pub fn setup_tracing() -> anyhow::Result<()> {
    let filter = EnvFilter::builder()
        .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
        .parse("")?;

    let subscriber = Registry::default()
        .with(filter)
        .with(tracing_logfmt::layer());
    dispatcher::set_global_default(Dispatch::new(subscriber))
        .context("Global logger has already been set!")?;
    Ok(())
}

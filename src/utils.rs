use chrono::{DateTime, Local};
use dialoguer::console::Emoji;
use std::io::BufRead;

pub const CHAT_GPT_MODEL_NAME: &str = "gpt-3.5-turbo";
pub const CHAT_GPT_KNOWLEDGE_CUTOFF: &str = "September 2021";
pub const VOICE_TO_TEXT_TRANSCRIBE_MODEL: &str = "whisper-1";

// Emojis
pub const ROBOT_EMOJI: Emoji = Emoji("🤖", "ChatGPT");
pub const QUESTION_MARK_EMOJI: Emoji = Emoji("❓", "User");

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

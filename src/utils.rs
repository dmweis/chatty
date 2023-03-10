use chrono::{DateTime, Local};
use dialoguer::console::Emoji;

pub const CHAT_MODEL_NAME: &str = "gpt-3.5-turbo";
pub const CHAT_MODEL_KNOWLEDGE_CUTOFF: &str = "2021";

pub const TRANSCRIBE_MODEL: &str = "whisper-1";

// Emojis
pub const ROBOT_EMOJI: Emoji = Emoji("🤖", "ChatGPT");
pub const QUESTION_MARK_EMOJI: Emoji = Emoji("❓", "User");

pub fn now() -> DateTime<Local> {
    Local::now()
}

pub fn now_rfc3339() -> String {
    now().to_rfc3339()
}

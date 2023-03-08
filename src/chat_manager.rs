use anyhow::{Context, Result};
use async_openai::{
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestMessageArgs,
        CreateChatCompletionRequestArgs, Role,
    },
    Client,
};
use chrono::prelude::{DateTime, Local};
use dialoguer::console::Term;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::configuration::get_project_dirs;

const CHAT_MODEL_NAME: &str = "gpt-3.5-turbo";
const CHAT_MODEL_KNOWLEDGE_CUTOFF: &str = "2021";

fn current_time() -> String {
    let dt: DateTime<Local> = Local::now();
    dt.to_rfc3339()
}

pub fn generate_system_instructions() -> HashMap<String, String> {
    let mut table = HashMap::new();

    let current_time_str = current_time();

    table.insert(
        String::from("default"),
        format!(
            "You are ChatGPT, a large language model trained by OpenAI. 
Answer as concisely as possible. Knowledge cutoff: {} Current date: {}",
            CHAT_MODEL_KNOWLEDGE_CUTOFF, current_time_str
        ),
    );

    table.insert(
        String::from("joi"),
        format!(
            "You are Joi. The cheerful and helpful AI assistant. 
Knowledge cutoff: {} Current date: {}",
            CHAT_MODEL_KNOWLEDGE_CUTOFF, current_time_str
        ),
    );

    table
}

pub struct ChatHistory {
    history: Vec<ChatCompletionRequestMessage>,
}

impl ChatHistory {
    pub fn new(prompt: &str) -> anyhow::Result<Self> {
        let history = vec![ChatCompletionRequestMessageArgs::default()
            .content(prompt)
            .role(Role::System)
            .build()?];
        Ok(Self { history })
    }

    pub async fn next_message(
        &mut self,
        user_message: &str,
        client: &Client,
    ) -> anyhow::Result<String> {
        let user_message = ChatCompletionRequestMessageArgs::default()
            .content(user_message)
            .role(Role::User)
            .build()?;

        self.history.push(user_message);

        let request = CreateChatCompletionRequestArgs::default()
            .model(CHAT_MODEL_NAME)
            .messages(self.history.clone())
            .build()?;

        let response = client.chat().create(request).await?;

        let added_response = ChatCompletionRequestMessageArgs::default()
            .content(response.choices[0].message.content.clone())
            .role(response.choices[0].message.role.clone())
            .build()?;

        self.history.push(added_response);

        Ok(response.choices[0].message.content.clone())
    }

    pub async fn next_message_stream_stdout(
        &mut self,
        user_message: &str,
        client: &Client,
        term: &Term,
    ) -> anyhow::Result<String> {
        let user_message = ChatCompletionRequestMessageArgs::default()
            .content(user_message)
            .role(Role::User)
            .build()?;

        self.history.push(user_message);

        let request = CreateChatCompletionRequestArgs::default()
            .model(CHAT_MODEL_NAME)
            .messages(self.history.clone())
            .build()?;

        let mut stream = client.chat().create_stream(request).await?;

        let mut response_role = None;
        let mut response_content_buffer = String::new();

        term.hide_cursor()?;

        // For reasons not documented in OpenAI docs / OpenAPI spec, the response of streaming call is different and doesn't include all the same fields.
        while let Some(result) = stream.next().await {
            let response = result?;
            // this ignores if there are multiple choices on the answer
            let delta = &response
                .choices
                .first()
                .context("No first choice on response")?
                .delta;
            // role and content are not guaranteed to be set on all deltas

            if let Some(role) = &delta.role {
                response_role = Some(role.clone());
            }

            if let Some(delta_content) = &delta.content {
                response_content_buffer.push_str(delta_content);
                term.write_str(delta_content)?;
            }
        }

        // this markdown thing doesn't work very well right now
        // re-render as markdown
        // consider adding slowdown for effect?
        // let lines = response_content_buffer.lines().count();
        // term.clear_last_lines(lines)?;
        // // this markdown is weird. Doesn't render correctly I think
        // let markdown = termimad::inline(&response_content_buffer);
        // term.write_line(&format!("{}", markdown))?;

        // empty new line after stream is done
        term.write_line("\n")?;

        term.show_cursor()?;

        let added_response = ChatCompletionRequestMessageArgs::default()
            .content(&response_content_buffer)
            .role(response_role.unwrap_or(Role::Assistant))
            .build()?;

        self.history.push(added_response);

        Ok(response_content_buffer)
    }

    pub fn save_to_file(&self) -> Result<()> {
        let project_dirs = get_project_dirs()?;
        let cache_dir = project_dirs.cache_dir();

        std::fs::create_dir_all(cache_dir).context("failed to crate user cache directory")?;

        let time = current_time();
        let file_path = cache_dir.join(format!("{time}.yaml"));

        let history_for_storage: Vec<ChatHistoryElement> =
            self.history.iter().map(|element| element.into()).collect();

        let history_storage = ChatHistoryStorage {
            messages: history_for_storage,
        };

        let file = std::fs::File::create(file_path)?;
        serde_yaml::to_writer(file, &history_storage)?;
        Ok(())
    }
}

impl From<&ChatCompletionRequestMessage> for ChatHistoryElement {
    fn from(source: &ChatCompletionRequestMessage) -> Self {
        Self {
            role: source.role.clone(),
            content: source.content.clone(),
            name: source.name.clone(),
        }
    }
}

impl From<ChatHistoryElement> for ChatCompletionRequestMessage {
    fn from(source: ChatHistoryElement) -> Self {
        Self {
            role: source.role,
            content: source.content,
            name: source.name,
        }
    }
}

/// used for storage
#[derive(Debug, Serialize, Deserialize, Clone)]
struct ChatHistoryStorage {
    /// message
    pub messages: Vec<ChatHistoryElement>,
}

/// used for storage
#[derive(Debug, Serialize, Deserialize, Clone)]
struct ChatHistoryElement {
    /// The role of the author of this message.
    pub role: Role,
    /// The contents of the message
    pub content: String,
    /// The name of the user in a multi-user chat
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

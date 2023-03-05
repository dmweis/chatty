use async_openai::{
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestMessageArgs,
        CreateChatCompletionRequestArgs, Role,
    },
    Client,
};

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
            .model("gpt-3.5-turbo")
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
}

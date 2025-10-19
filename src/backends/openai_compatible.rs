use super::{LlmBackend, LlmResponse};
use crate::conversations::{Conversation, ConversationMessage, ToolCall};
use crate::tools::ToolRegistry;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct OpenAICompatibleConfig {
    pub name: String,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub temperature: Option<f32>,
}

impl Default for OpenAICompatibleConfig {
    fn default() -> Self {
        Self {
            name: "openai".to_string(),
            api_key: String::new(),
            model: "gpt-4".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            temperature: None,
        }
    }
}

pub struct OpenAICompatibleBackend {
    client: reqwest::Client,
    config: OpenAICompatibleConfig,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ConversationMessage>,
    max_completion_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Choice {
    message: Option<ResponseMessage>,
    delta: Option<ResponseMessage>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ResponseMessage {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCall>>,
}

impl OpenAICompatibleBackend {
    pub fn new(config: OpenAICompatibleConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;
        Ok(Self { client, config })
    }

    fn create_request(&self, message: &str) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: vec![ConversationMessage {
                role: "user".to_string(),
                content: Some(message.to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            }],
            max_completion_tokens: 4096,
            temperature: self.config.temperature,
            tools: None,
            tool_choice: None,
        }
    }

    fn create_request_with_tools(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> ChatCompletionRequest {
        let tool_schemas = tools.get_tool_schemas();
        let has_tools = !tool_schemas.is_empty();

        ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: conversation.get_messages_for_api().clone(),
            max_completion_tokens: 4096,
            temperature: self.config.temperature,
            tools: if has_tools { Some(tool_schemas) } else { None },
            tool_choice: if has_tools {
                Some("auto".to_string())
            } else {
                None
            },
        }
    }
}

#[async_trait]
impl LlmBackend for OpenAICompatibleBackend {
    async fn send_message(&self, message: &str) -> Result<String> {
        if self.config.api_key.is_empty() {
            anyhow::bail!(
                "{} API key not configured. Set it with: hoosh config set {}_api_key <your_key>",
                self.config.name,
                self.config.name
            );
        }

        let request = self.create_request(message);
        let url = format!("{}/chat/completions", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&request)
            .send()
            .await
            .context(format!("Failed to send request to {}", self.config.name))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("{} API error {}: {}", self.config.name, status, error_text);
        }

        let response_data: ChatCompletionResponse = response.json().await.context(format!(
            "Failed to parse response from {}",
            self.config.name
        ))?;

        response_data
            .choices
            .first()
            .and_then(|choice| choice.message.as_ref())
            .and_then(|message| message.content.as_ref())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No response from {}", self.config.name))
    }

    async fn send_message_with_tools(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> Result<LlmResponse> {
        if self.config.api_key.is_empty() {
            anyhow::bail!(
                "{} API key not configured. Set it with: hoosh config set {}_api_key <your_key>",
                self.config.name,
                self.config.name
            );
        }

        let request = self.create_request_with_tools(conversation, tools);
        let url = format!("{}/chat/completions", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&request)
            .send()
            .await
            .context(format!("Failed to send request to {}", self.config.name))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("{} API error {}: {}", self.config.name, status, error_text);
        }

        let response_data: ChatCompletionResponse = response.json().await.context(format!(
            "Failed to parse response from {}",
            self.config.name
        ))?;

        if let Some(choice) = response_data.choices.first() {
            if let Some(message) = &choice.message {
                if let Some(tool_calls) = &message.tool_calls {
                    // Response contains tool calls
                    return Ok(LlmResponse::with_tool_calls(
                        message.content.clone(),
                        tool_calls.clone(),
                    ));
                } else if let Some(content) = &message.content {
                    // Response contains only content
                    return Ok(LlmResponse::content_only(content.clone()));
                }
            }
        }

        anyhow::bail!("No valid response from {}", self.config.name)
    }

    fn backend_name(&self) -> &str {
        &self.config.name
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }
}

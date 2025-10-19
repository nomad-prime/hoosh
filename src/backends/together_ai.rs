use super::{LlmBackend, LlmResponse};
use crate::conversations::{Conversation, ConversationMessage, ToolCall};
use crate::tools::ToolRegistry;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct TogetherAiConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}

impl Default for TogetherAiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "meta-llama/Llama-2-7b-chat-hf".to_string(),
            base_url: "https://api.together.xyz/v1".to_string(),
        }
    }
}

pub struct TogetherAiBackend {
    client: reqwest::Client,
    config: TogetherAiConfig,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ConversationMessage>,
    max_tokens: Option<u32>,
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

impl TogetherAiBackend {
    pub fn new(config: TogetherAiConfig) -> Result<Self> {
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
            max_tokens: Some(4096),
            temperature: Some(0.7),
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
            max_tokens: Some(4096),
            temperature: Some(0.7),
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
impl LlmBackend for TogetherAiBackend {
    async fn send_message(&self, message: &str) -> Result<String> {
        if self.config.api_key.is_empty() {
            anyhow::bail!("Together AI API key not configured. Set it with: hoosh config set together_ai_api_key <your_key>");
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
            .context("Failed to send request to Together AI")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Together AI API error {}: {}", status, error_text);
        }

        let response_data: ChatCompletionResponse = response
            .json()
            .await
            .context("Failed to parse response from Together AI")?;

        response_data
            .choices
            .first()
            .and_then(|choice| choice.message.as_ref())
            .and_then(|message| message.content.as_ref())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No response from Together AI"))
    }

    async fn send_message_with_tools(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> Result<LlmResponse> {
        if self.config.api_key.is_empty() {
            anyhow::bail!("Together AI API key not configured. Set it with: hoosh config set together_ai_api_key <your_key>");
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
            .context("Failed to send request to Together AI")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Together AI API error {}: {}", status, error_text);
        }

        let response_data: ChatCompletionResponse = response
            .json()
            .await
            .context("Failed to parse response from Together AI")?;

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

        anyhow::bail!("No valid response from Together AI")
    }

    fn backend_name(&self) -> &str {
        "together_ai"
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }
}

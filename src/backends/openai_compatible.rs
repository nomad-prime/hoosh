use super::{LlmBackend, LlmResponse};
use crate::backends::llm_error::LlmError;
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
        let mut client_builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .connect_timeout(std::time::Duration::from_secs(30));

        // Configure HTTP proxy if environment variables are set
        if let Ok(http_proxy) = std::env::var("HTTP_PROXY") {
            if let Ok(proxy) = reqwest::Proxy::http(&http_proxy) {
                client_builder = client_builder.proxy(proxy);
            }
        }

        // Configure HTTPS proxy if environment variables are set
        if let Ok(https_proxy) = std::env::var("HTTPS_PROXY") {
            if let Ok(proxy) = reqwest::Proxy::https(&https_proxy) {
                client_builder = client_builder.proxy(proxy);
            }
        }

        let client = client_builder
            .build()
            .context("Failed to build HTTP client")?;
        Ok(Self { client, config })
    }

    fn http_error_to_llm_error(status: reqwest::StatusCode, error_text: String) -> LlmError {
        let status_code = status.as_u16();

        match status_code {
            429 => {
                // Try to parse retry-after header
                // We can't access the header here, but we'll handle it in the request method
                LlmError::RateLimit {
                    retry_after: None,
                    message: error_text,
                }
            }
            500..=599 => LlmError::ServerError {
                status: status_code,
                message: error_text,
            },
            401 | 403 => LlmError::AuthenticationError {
                message: error_text,
            },
            _ => LlmError::Other {
                message: format!("API error {}: {}", status_code, error_text),
            },
        }
    }

    async fn send_message_attempt(&self, message: &str) -> Result<String, LlmError> {
        if self.config.api_key.is_empty() {
            return Err(LlmError::AuthenticationError {
                message: format!("{} API key not configured. Set it with: hoosh config set {}_api_key <your_key>",
                    self.config.name, self.config.name)
            });
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
            .map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })?;

        let status = response.status();
        if !status.is_success() {
            // Clone response before consuming it to get headers
            let headers = response.headers().clone();
            let error_text = response.text().await.unwrap_or_default();

            // Handle rate limit with retry-after header
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let retry_after = headers
                    .get("retry-after")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());
                return Err(LlmError::RateLimit {
                    retry_after,
                    message: error_text,
                });
            }

            return Err(Self::http_error_to_llm_error(status, error_text));
        }

        let response_data: ChatCompletionResponse =
            response.json().await.map_err(|e| LlmError::Other {
                message: format!("Failed to parse response: {}", e),
            })?;

        response_data
            .choices
            .first()
            .and_then(|choice| choice.message.as_ref())
            .and_then(|message| message.content.as_ref())
            .cloned()
            .ok_or_else(|| LlmError::Other {
                message: format!("No response from {}", self.config.name),
            })
    }

    async fn send_message_with_tools_attempt(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> Result<LlmResponse, LlmError> {
        if self.config.api_key.is_empty() {
            return Err(LlmError::AuthenticationError {
                message: format!("{} API key not configured. Set it with: hoosh config set {}_api_key <your_key>",
                    self.config.name, self.config.name)
            });
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
            .map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })?;

        let status = response.status();
        if !status.is_success() {
            // Clone response before consuming it to get headers
            let headers = response.headers().clone();
            let error_text = response.text().await.unwrap_or_default();

            // Handle rate limit with retry-after header
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let retry_after = headers
                    .get("retry-after")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());
                return Err(LlmError::RateLimit {
                    retry_after,
                    message: error_text,
                });
            }

            return Err(Self::http_error_to_llm_error(status, error_text));
        }

        let response_data: ChatCompletionResponse =
            response.json().await.map_err(|e| LlmError::Other {
                message: format!("Failed to parse response: {}", e),
            })?;

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

        Err(LlmError::Other {
            message: format!("No valid response from {}", self.config.name),
        })
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
        self.send_message_attempt(message)
            .await
            .map_err(|e| anyhow::anyhow!(e.user_message()))
    }

    async fn send_message_with_tools(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> Result<LlmResponse> {
        self.send_message_with_tools_attempt(conversation, tools)
            .await
            .map_err(|e| anyhow::anyhow!(e.user_message()))
    }

    fn backend_name(&self) -> &str {
        &self.config.name
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }
}

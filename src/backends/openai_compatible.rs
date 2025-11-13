use super::{LlmBackend, LlmResponse, RequestExecutor};
use crate::agent::{Conversation, ConversationMessage, ToolCall};
use crate::backends::llm_error::LlmError;
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
    pub chat_api: String,
}

impl Default for OpenAICompatibleConfig {
    fn default() -> Self {
        Self {
            name: "openai".to_string(),
            api_key: String::new(),
            model: "gpt-4".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            chat_api: "/chat/completions".to_string(),
            temperature: None,
        }
    }
}

pub struct OpenAICompatibleBackend {
    client: reqwest::Client,
    config: OpenAICompatibleConfig,
    default_executor: RequestExecutor,
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
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
    // New response format fields
    #[serde(default)]
    output: Option<Vec<Output>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Output {
    #[serde(default)]
    content: Option<Vec<ContentBlock>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "output_text")]
    OutputText { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

#[derive(Debug, Deserialize, Serialize)]
struct Usage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Choice {
    message: Option<ResponseMessage>,
    delta: Option<ResponseMessage>,
    #[serde(default)]
    finish_reason: Option<String>,
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
        if let Ok(http_proxy) = std::env::var("HTTP_PROXY")
            && let Ok(proxy) = reqwest::Proxy::http(&http_proxy)
        {
            client_builder = client_builder.proxy(proxy);
        }

        // Configure HTTPS proxy if environment variables are set
        if let Ok(https_proxy) = std::env::var("HTTPS_PROXY")
            && let Ok(proxy) = reqwest::Proxy::https(&https_proxy)
        {
            client_builder = client_builder.proxy(proxy);
        }

        let client = client_builder
            .build()
            .context("Failed to build HTTP client")?;

        let default_executor = RequestExecutor::new(3, "OpenAI-compatible API request".to_string());

        Ok(Self {
            client,
            config,
            default_executor,
        })
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
                message: format!(
                    "{} API key not configured. Set it with: hoosh config set {}_api_key <your_key>",
                    self.config.name, self.config.name
                ),
            });
        }

        let request = self.create_request(message);
        let url = format!("{}{}", self.config.base_url, self.config.chat_api);

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
                message: format!(
                    "{} API key not configured. Set it with: hoosh config set {}_api_key <your_key>",
                    self.config.name, self.config.name
                ),
            });
        }

        let request = self.create_request_with_tools(conversation, tools);
        let url = format!("{}{}", self.config.base_url, self.config.chat_api);

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


        let response_text = response.text().await.map_err(|e| LlmError::Other {
            message: format!("Failed to read response: {}", e),
        })?;


        let response_data: ChatCompletionResponse =
            serde_json::from_str(&response_text).map_err(|e| LlmError::Other {
                message: format!("Failed to parse response: {}", e),
            })?;

        // Check if response was truncated due to length limit
        if let Some(choice) = response_data.choices.first() {
            if let Some(finish_reason) = &choice.finish_reason {
                if finish_reason == "length" {
                    return Err(LlmError::RecoverableByLlm {
                        message: "Your response was cut off because it exceeded the maximum token limit. Please provide a shorter, more concise response. If you were writing a large file or tool call, break it into smaller parts.".to_string(),
                    });
                }
            }
        }

        // Extract tokens - handle both old and new API formats
        let (input_tokens, output_tokens) = if let Some(usage) = response_data.usage {
            let input = if usage.input_tokens > 0 {
                usage.input_tokens as usize
            } else {
                usage.prompt_tokens as usize
            };
            let output = if usage.output_tokens > 0 {
                usage.output_tokens as usize
            } else {
                usage.completion_tokens as usize
            };
            (input, output)
        } else {
            (0, 0)
        };

        // Try new response format first (with output field)
        if let Some(outputs) = response_data.output
            && let Some(output) = outputs.first()
            && let Some(content_blocks) = &output.content
        {
            let mut text_parts = Vec::new();
            let mut tool_calls = Vec::new();

            for block in content_blocks {
                match block {
                    ContentBlock::OutputText { text } => {
                        text_parts.push(text.clone());
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        tool_calls.push(ToolCall {
                            id: id.clone(),
                            r#type: "function".to_string(),
                            function: crate::agent::ToolFunction {
                                name: name.clone(),
                                arguments: input.to_string(),
                            },
                        });
                    }
                }
            }

            if !tool_calls.is_empty() {
                let content = if text_parts.is_empty() {
                    None
                } else {
                    Some(text_parts.join("\n"))
                };
                return Ok(LlmResponse::with_tool_calls(content, tool_calls)
                    .with_tokens(input_tokens, output_tokens));
            } else if !text_parts.is_empty() {
                return Ok(LlmResponse::content_only(text_parts.join("\n"))
                    .with_tokens(input_tokens, output_tokens));
            }
        }

        // Fall back to traditional chat completion format
        if let Some(choice) = response_data.choices.first()
            && let Some(message) = &choice.message
        {
            if let Some(tool_calls) = &message.tool_calls {
                // Response contains tool calls
                return Ok(LlmResponse::with_tool_calls(
                    message.content.clone(),
                    tool_calls.clone(),
                )
                .with_tokens(input_tokens, output_tokens));
            } else if let Some(content) = &message.content {
                // Response contains only content
                return Ok(LlmResponse::content_only(content.clone())
                    .with_tokens(input_tokens, output_tokens));
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
        self.default_executor
            .execute(|| async { self.send_message_attempt(message).await }, None)
            .await
            .map_err(|e| anyhow::Error::new(e))
    }

    async fn send_message_with_tools(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> Result<LlmResponse, LlmError> {
        self.default_executor
            .execute(
                || async {
                    self.send_message_with_tools_attempt(conversation, tools)
                        .await
                },
                None,
            )
            .await
    }

    fn backend_name(&self) -> &str {
        &self.config.name
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }

    fn pricing(&self) -> Option<crate::backends::TokenPricing> {
        let pricing = match self.config.model.as_str() {
            "gpt-4o" | "gpt-4o-2024-11-20" | "gpt-4o-2024-08-06" | "gpt-4o-2024-05-13" => {
                crate::backends::TokenPricing {
                    input_per_million: 2.5,
                    output_per_million: 10.0,
                }
            }
            "gpt-4o-mini" | "gpt-4o-mini-2024-07-18" => crate::backends::TokenPricing {
                input_per_million: 0.15,
                output_per_million: 0.6,
            },
            "o1" | "o1-2024-12-17" => crate::backends::TokenPricing {
                input_per_million: 15.0,
                output_per_million: 60.0,
            },
            "o1-mini" | "o1-mini-2024-09-12" => crate::backends::TokenPricing {
                input_per_million: 3.0,
                output_per_million: 12.0,
            },
            _ => return None,
        };
        Some(pricing)
    }

    async fn send_message_with_events(
        &self,
        message: &str,
        event_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::agent::AgentEvent>>,
    ) -> Result<String> {
        self.default_executor
            .execute(
                || async { self.send_message_attempt(message).await },
                event_tx,
            )
            .await
            .map_err(|e| anyhow::Error::new(e))
    }

    async fn send_message_with_tools_and_events(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
        event_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::agent::AgentEvent>>,
    ) -> Result<LlmResponse, LlmError> {
        self.default_executor
            .execute(
                || async {
                    self.send_message_with_tools_attempt(conversation, tools)
                        .await
                },
                event_tx,
            )
            .await
    }
}

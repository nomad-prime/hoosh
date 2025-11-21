use super::{LlmBackend, LlmResponse, RequestExecutor};
use crate::agent::{Conversation, ConversationMessage, ToolCall};
use crate::backends::llm_error::LlmError;
use crate::tools::ToolRegistry;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

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
    default_executor: RequestExecutor,
    pricing: Arc<RwLock<Option<crate::backends::TokenPricing>>>,
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
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
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

#[derive(Debug, Deserialize)]
struct ModelInfo {
    id: String,
    #[serde(default)]
    pricing: Option<ModelPricing>,
}

#[derive(Debug, Deserialize)]
struct ModelPricing {
    input: f64,
    output: f64,
}

impl TogetherAiBackend {
    pub fn new(config: TogetherAiConfig) -> Result<Self> {
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

        let default_executor = RequestExecutor::new(3, "Together AI API request".to_string());

        Ok(Self {
            client,
            config,
            default_executor,
            pricing: Arc::new(RwLock::new(None)),
        })
    }

    async fn fetch_and_cache_pricing(&self) -> Result<()> {
        let url = format!("{}/models", self.config.base_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .send()
            .await
            .context("Failed to fetch pricing from Together AI")?;

        let models: Vec<ModelInfo> = response
            .json()
            .await
            .context("Failed to parse models response")?;

        if let Some(model_info) = models.iter().find(|m| m.id == self.config.model)
            && let Some(pricing_info) = &model_info.pricing
        {
            let pricing = crate::backends::TokenPricing {
                input_per_million: pricing_info.input,
                output_per_million: pricing_info.output,
            };

            let mut pricing_write = self.pricing.write().await;
            *pricing_write = Some(pricing);
        }

        Ok(())
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
                message: "Together AI API key not configured. Set it with: hoosh config set together_ai_api_key <your_key>".to_string()
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

        let response_str = response.text().await.map_err(|e| LlmError::Other {
            message: format!("Failed to read response: {}", e),
        })?;

        let response_data: ChatCompletionResponse = serde_json::from_str(&response_str.to_string())
            .map_err(|e| LlmError::RecoverableByLlm {
                message: format!("Failed to parse response: {}, {}", e, response_str),
            })?;

        response_data
            .choices
            .first()
            .and_then(|choice| choice.message.as_ref())
            .and_then(|message| message.content.as_ref())
            .cloned()
            .ok_or_else(|| LlmError::Other {
                message: "No response from Together AI".to_string(),
            })
    }

    async fn send_message_with_tools_attempt(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> Result<LlmResponse, LlmError> {
        if self.config.api_key.is_empty() {
            return Err(LlmError::AuthenticationError {
                message: "Together AI API key not configured. Set it with: hoosh config set together_ai_api_key <your_key>".to_string()
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

        let response_str = response.text().await.map_err(|e| LlmError::Other {
            message: format!("Failed to read response: {}", e),
        })?;

        let response_data: ChatCompletionResponse = serde_json::from_str(&response_str.to_string())
            .map_err(|e| LlmError::RecoverableByLlm {
                message: format!("Failed to parse response: {}, {}", e, response_str),
            })?;

        let (input_tokens, output_tokens) = if let Some(usage) = response_data.usage {
            (
                usage.prompt_tokens as usize,
                usage.completion_tokens as usize,
            )
        } else {
            (0, 0)
        };

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
            message: "No valid response from Together AI".to_string(),
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
            max_tokens: Some(8192),
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
            max_tokens: Some(8192),
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
        self.default_executor
            .execute(|| async { self.send_message_attempt(message).await }, None)
            .await
            .map_err(anyhow::Error::new)
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
            .map_err(anyhow::Error::new)
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

    fn backend_name(&self) -> &str {
        "together_ai"
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }

    async fn initialize(&self) -> Result<()> {
        // Fetch pricing at initialization
        self.fetch_and_cache_pricing().await?;
        Ok(())
    }

    fn pricing(&self) -> Option<crate::backends::TokenPricing> {
        // Simple read from memory
        self.pricing.try_read().ok().and_then(|guard| *guard)
    }
}

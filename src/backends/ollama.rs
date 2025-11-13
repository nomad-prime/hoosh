use super::{LlmBackend, LlmResponse, RequestExecutor};
use crate::agent::{Conversation, ConversationMessage, ToolCall};
use crate::backends::llm_error::LlmError;
use crate::tools::ToolRegistry;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_OLLAMA_MODEL: &str = "llama3";

const DEFAULT_MAX_TOKENS: u32 = 4096;

#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub name: String,
    pub model: String,
    pub base_url: String,
    pub temperature: Option<f32>,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            name: "ollama".to_string(),
            model: DEFAULT_OLLAMA_MODEL.to_string(),
            base_url: DEFAULT_OLLAMA_BASE_URL.to_string(),
            temperature: None,
        }
    }
}

pub struct OllamaBackend {
    client: reqwest::Client,
    config: OllamaConfig,
    default_executor: RequestExecutor,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<ModelOptions>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
}

#[derive(Debug, Serialize)]
struct OllamaMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OllamaToolCallRequest>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct OllamaToolCallRequest {
    function: OllamaToolFunctionRequest,
}

#[derive(Debug, Serialize)]
struct OllamaToolFunctionRequest {
    name: String,
    arguments: Value,
}

impl From<&ConversationMessage> for OllamaMessage {
    fn from(msg: &ConversationMessage) -> Self {
        let tool_calls = msg.tool_calls.as_ref().map(|calls| {
            calls
                .iter()
                .map(|call| OllamaToolCallRequest {
                    function: OllamaToolFunctionRequest {
                        name: call.function.name.clone(),
                        arguments: serde_json::from_str(&call.function.arguments)
                            .unwrap_or(Value::Object(serde_json::Map::new())),
                    },
                })
                .collect()
        });

        OllamaMessage {
            role: msg.role.clone(),
            content: msg.content.clone(),
            tool_calls,
            tool_call_id: msg.tool_call_id.clone(),
            name: msg.name.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ModelOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_ctx: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: ResponseMessage,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

// Ollama's tool call format (different from OpenAI/standard format)
#[derive(Debug, Deserialize)]
struct OllamaToolCall {
    function: OllamaToolFunction,
}

#[derive(Debug, Deserialize)]
struct OllamaToolFunction {
    name: String,
    #[serde(default)]
    arguments: Value,
}

impl OllamaToolCall {
    fn to_standard_tool_call(&self, index: usize) -> ToolCall {
        ToolCall {
            id: format!("call_{}", index),
            r#type: "function".to_string(),
            function: crate::agent::ToolFunction {
                name: self.function.name.clone(),
                arguments: self.function.arguments.to_string(),
            },
        }
    }
}

impl OllamaBackend {
    pub fn new(config: OllamaConfig) -> Result<Self> {
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

        let default_executor = RequestExecutor::new(3, "Ollama API request".to_string());

        Ok(Self {
            client,
            config,
            default_executor,
        })
    }

    async fn send_message_attempt(&self, message: &str) -> Result<String, LlmError> {
        let request = self.create_request(message);
        let url = format!("{}/api/chat", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmError::Other {
                message: format!("Ollama API error {}:", status.as_u16()) + &error_text,
            });
        }

        // Read response as text first to provide better error messages
        let response_text = response.text().await.map_err(|e| LlmError::Other {
            message: format!("Failed to read Ollama response body: {}", e),
        })?;

        let response_data: ChatResponse =
            serde_json::from_str(&response_text).map_err(|e| LlmError::Other {
                message: format!(
                    "Failed to parse Ollama response: {}\nRaw response: {}",
                    e, response_text
                ),
            })?;

        Ok(response_data.message.content)
    }

    async fn send_message_with_tools_attempt(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> Result<LlmResponse, LlmError> {
        let tool_schemas = tools.get_tool_schemas();

        let request = self.create_request_with_tools(conversation, tool_schemas);
        let url = format!("{}/api/chat", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmError::Other {
                message: format!("Ollama API error {}:", status.as_u16()) + &error_text,
            });
        }

        let response_text = response.text().await.map_err(|e| LlmError::Other {
            message: format!("Failed to read Ollama response body: {}", e),
        })?;

        let response_data: ChatResponse =
            serde_json::from_str(&response_text).map_err(|e| LlmError::Other {
                message: format!(
                    "Failed to parse Ollama response: {}\nRaw response: {}",
                    e, response_text
                ),
            })?;

        let tool_calls = response_data.message.tool_calls.map(|calls| {
            calls
                .into_iter()
                .enumerate()
                .map(|(i, call)| call.to_standard_tool_call(i))
                .collect::<Vec<ToolCall>>()
        });

        let input_tokens = response_data
            .prompt_eval_count
            .map(|c| c as usize)
            .unwrap_or(0);
        let output_tokens = response_data.eval_count.map(|c| c as usize).unwrap_or(0);

        let response = if let Some(tool_calls) = tool_calls {
            LlmResponse::with_tool_calls(Some(response_data.message.content), tool_calls)
        } else {
            LlmResponse::content_only(response_data.message.content)
        };

        Ok(response.with_tokens(input_tokens, output_tokens))
    }

    fn create_request(&self, message: &str) -> ChatRequest {
        let options = self.create_model_options();

        let user_msg = ConversationMessage {
            role: "user".to_string(),
            content: Some(message.to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };

        ChatRequest {
            model: self.config.model.clone(),
            messages: vec![OllamaMessage::from(&user_msg)],
            options: Some(options),
            tools: None,
            stream: false,
        }
    }

    fn create_request_with_tools(
        &self,
        conversation: &Conversation,
        tools: Vec<Value>,
    ) -> ChatRequest {
        let options = self.create_model_options();

        // Convert all messages to Ollama format
        let messages: Vec<OllamaMessage> = conversation
            .get_messages_for_api()
            .iter()
            .map(OllamaMessage::from)
            .collect();

        ChatRequest {
            model: self.config.model.clone(),
            messages,
            options: Some(options),
            stream: false,
            tools: Some(tools),
        }
    }

    fn create_model_options(&self) -> ModelOptions {
        ModelOptions {
            seed: None,
            temperature: self.config.temperature,
            top_k: None,
            top_p: None,
            min_p: None,
            stop: None,
            num_ctx: None,
            num_predict: Some(DEFAULT_MAX_TOKENS as i32),
        }
    }
}

#[async_trait]
impl LlmBackend for OllamaBackend {
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

    fn backend_name(&self) -> &str {
        &self.config.name
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }
}

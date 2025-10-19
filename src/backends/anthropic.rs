use super::{LlmBackend, LlmResponse};
use crate::conversations::{Conversation, ConversationMessage, ToolCall};
use crate::tools::ToolRegistry;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "claude-sonnet-4.5".to_string(),
            base_url: "https://api.anthropic.com/v1".to_string(),
        }
    }
}

pub struct AnthropicBackend {
    client: reqwest::Client,
    config: AnthropicConfig,
}

#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContent,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MessagesResponse {
    id: String,
    content: Vec<ContentBlock>,
    model: String,
    stop_reason: Option<String>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

impl AnthropicBackend {
    pub fn new(config: AnthropicConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;
        Ok(Self { client, config })
    }

    fn convert_messages(
        &self,
        messages: &[ConversationMessage],
    ) -> (Option<String>, Vec<AnthropicMessage>) {
        let mut system_prompt = None;
        let mut anthropic_messages: Vec<AnthropicMessage> = Vec::new();

        for msg in messages {
            if msg.role == "system" {
                // Anthropic uses a separate system parameter
                if let Some(content) = &msg.content {
                    system_prompt = Some(content.clone());
                }
            } else {
                // Anthropic only accepts "user" or "assistant" roles
                // Convert "tool" role to "user" (tool results are user messages)
                let role = if msg.role == "tool" {
                    "user".to_string()
                } else {
                    msg.role.clone()
                };

                let content = if let Some(tool_calls) = &msg.tool_calls {
                    // Convert tool calls to Anthropic format
                    // Assistant messages can have both text and tool_use blocks
                    let mut blocks: Vec<ContentBlock> = Vec::new();

                    // Add text block if content exists
                    if let Some(text) = &msg.content {
                        if !text.is_empty() {
                            blocks.push(ContentBlock::Text { text: text.clone() });
                        }
                    }

                    // Add tool_use blocks
                    for tc in tool_calls {
                        blocks.push(ContentBlock::ToolUse {
                            id: tc.id.clone(),
                            name: tc.function.name.clone(),
                            input: serde_json::from_str(&tc.function.arguments)
                                .unwrap_or(Value::Object(serde_json::Map::new())),
                        });
                    }

                    AnthropicContent::Blocks(blocks)
                } else if let Some(tool_call_id) = &msg.tool_call_id {
                    // Tool result message
                    let blocks = vec![ContentBlock::ToolResult {
                        tool_use_id: tool_call_id.clone(),
                        content: msg.content.clone().unwrap_or_default(),
                    }];
                    AnthropicContent::Blocks(blocks)
                } else {
                    // Regular text message
                    AnthropicContent::Text(msg.content.clone().unwrap_or_default())
                };

                // Check if we need to merge with the previous message (same role)
                if let Some(last_msg) = anthropic_messages.last_mut() {
                    if last_msg.role == role {
                        // Merge content blocks to maintain role alternation
                        let new_blocks = match content {
                            AnthropicContent::Text(text) => vec![ContentBlock::Text { text }],
                            AnthropicContent::Blocks(blocks) => blocks,
                        };

                        // Convert last message content to blocks if needed
                        let mut merged_blocks = match &last_msg.content {
                            AnthropicContent::Text(text) => {
                                vec![ContentBlock::Text { text: text.clone() }]
                            }
                            AnthropicContent::Blocks(blocks) => blocks.clone(),
                        };

                        merged_blocks.extend(new_blocks);
                        last_msg.content = AnthropicContent::Blocks(merged_blocks);
                        continue;
                    }
                }

                anthropic_messages.push(AnthropicMessage { role, content });
            }
        }

        (system_prompt, anthropic_messages)
    }

    fn create_request(&self, message: &str) -> MessagesRequest {
        MessagesRequest {
            model: self.config.model.clone(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: AnthropicContent::Text(message.to_string()),
            }],
            max_tokens: 4096,
            system: None,
            temperature: Some(0.7),
            tools: None,
        }
    }

    fn convert_tool_schemas(&self, tool_schemas: Vec<Value>) -> Vec<Value> {
        // Convert from OpenAI format to Anthropic format
        tool_schemas
            .into_iter()
            .filter_map(|schema| {
                // OpenAI format: { "type": "function", "function": { "name": "...", "description": "...", "parameters": {...} } }
                // Anthropic format: { "name": "...", "description": "...", "input_schema": {...} }
                if let Some(function) = schema.get("function") {
                    Some(serde_json::json!({
                        "name": function.get("name")?,
                        "description": function.get("description")?,
                        "input_schema": function.get("parameters")?
                    }))
                } else {
                    // Already in correct format or invalid
                    Some(schema)
                }
            })
            .collect()
    }

    fn create_request_with_tools(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> MessagesRequest {
        let (system_prompt, messages) = self.convert_messages(conversation.get_messages_for_api());
        let tool_schemas = self.convert_tool_schemas(tools.get_tool_schemas());
        let has_tools = !tool_schemas.is_empty();

        MessagesRequest {
            model: self.config.model.clone(),
            messages,
            max_tokens: 4096,
            system: system_prompt,
            temperature: Some(0.7),
            tools: if has_tools { Some(tool_schemas) } else { None },
        }
    }

    async fn send_request(&self, request: &MessagesRequest) -> Result<MessagesResponse> {
        let url = format!("{}/messages", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(request)
            .send()
            .await
            .context("Failed to send request to Anthropic")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error {}: {}", status, error_text);
        }

        let response_data: MessagesResponse = response
            .json()
            .await
            .context("Failed to parse response from Anthropic")?;

        Ok(response_data)
    }

    fn extract_text_from_response(&self, response: MessagesResponse) -> Option<String> {
        let mut text_parts = Vec::new();
        for block in response.content {
            if let ContentBlock::Text { text } = block {
                text_parts.push(text);
            }
        }
        if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join("\n"))
        }
    }

    fn extract_llm_response(&self, response: MessagesResponse) -> LlmResponse {
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in response.content {
            match block {
                ContentBlock::Text { text } => {
                    text_parts.push(text);
                }
                ContentBlock::ToolUse { id, name, input } => {
                    // Convert Anthropic tool use to our ToolCall format
                    tool_calls.push(ToolCall {
                        id,
                        r#type: "function".to_string(),
                        function: crate::conversations::ToolFunction {
                            name,
                            arguments: input.to_string(),
                        },
                    });
                }
                _ => {}
            }
        }

        if !tool_calls.is_empty() {
            let content = if text_parts.is_empty() {
                None
            } else {
                Some(text_parts.join("\n"))
            };
            LlmResponse::with_tool_calls(content, tool_calls)
        } else {
            LlmResponse::content_only(text_parts.join("\n"))
        }
    }
}

#[async_trait]
impl LlmBackend for AnthropicBackend {
    async fn send_message(&self, message: &str) -> Result<String> {
        if self.config.api_key.is_empty() {
            anyhow::bail!("Anthropic API key not configured. Set it with: hoosh config set anthropic_api_key <your_key>");
        }

        let request = self.create_request(message);
        let response = self.send_request(&request).await?;

        self.extract_text_from_response(response)
            .ok_or_else(|| anyhow::anyhow!("No text content in response from Anthropic"))
    }

    async fn send_message_with_tools(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> Result<LlmResponse> {
        if self.config.api_key.is_empty() {
            anyhow::bail!("Anthropic API key not configured. Set it with: hoosh config set anthropic_api_key <your_key>");
        }

        let request = self.create_request_with_tools(conversation, tools);
        let response = self.send_request(&request).await?;

        Ok(self.extract_llm_response(response))
    }

    fn backend_name(&self) -> &str {
        "anthropic"
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }
}

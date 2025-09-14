use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use super::{LlmBackend, StreamResponse};

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
    messages: Vec<ChatMessage>,
    stream: bool,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<ChatMessage>,
    delta: Option<ChatMessage>,
}

#[derive(Debug, Deserialize)]
struct StreamingResponse {
    choices: Vec<Choice>,
}

impl TogetherAiBackend {
    pub fn new(config: TogetherAiConfig) -> Result<Self> {
        let client = reqwest::Client::new();
        Ok(Self { client, config })
    }

    fn create_request(&self, message: &str, stream: bool) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: message.to_string(),
            }],
            stream,
            max_tokens: Some(4096),
            temperature: Some(0.7),
        }
    }
}

#[async_trait]
impl LlmBackend for TogetherAiBackend {
    async fn send_message(&self, message: &str) -> Result<String> {
        if self.config.api_key.is_empty() {
            anyhow::bail!("Together AI API key not configured. Set it with: hoosh config set together_ai_api_key <your_key>");
        }

        let request = self.create_request(message, false);
        let url = format!("{}/chat/completions", self.config.base_url);

        let response = self.client
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
            .map(|message| message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("No response from Together AI"))
    }

    async fn stream_message(&self, message: &str) -> Result<StreamResponse> {
        if self.config.api_key.is_empty() {
            anyhow::bail!("Together AI API key not configured. Set it with: hoosh config set together_ai_api_key <your_key>");
        }

        let request = self.create_request(message, true);
        let url = format!("{}/chat/completions", self.config.base_url);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&request)
            .send()
            .await
            .context("Failed to send streaming request to Together AI")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Together AI API error {}: {}", status, error_text);
        }

        // Handle Server-Sent Events (SSE) streaming
        let stream = response
            .bytes_stream()
            .map_err(|e| anyhow::anyhow!("Stream error: {}", e))
            .map(|chunk| {
                let chunk = chunk?;
                let text = String::from_utf8_lossy(&chunk);

                // Parse SSE format: "data: {...}\n\n"
                for line in text.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            continue;
                        }

                        if let Ok(streaming_response) = serde_json::from_str::<StreamingResponse>(data) {
                            if let Some(choice) = streaming_response.choices.first() {
                                if let Some(delta) = &choice.delta {
                                    if !delta.content.is_empty() {
                                        return Ok(delta.content.clone());
                                    }
                                }
                            }
                        }
                    }
                }

                Ok(String::new())
            })
            .try_filter(|s| {
                // Filter out empty strings
                futures_util::future::ready(!s.is_empty())
            });

        Ok(Box::pin(stream))
    }

    fn backend_name(&self) -> &'static str {
        "together_ai"
    }
}

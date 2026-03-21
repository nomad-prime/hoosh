use super::*;
use crate::agent::Conversation;
use crate::backends::{LlmBackend, LlmError};
use crate::tools::ToolRegistry;
use serde_json::json;
use wiremock::matchers::{header, header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn create_test_config() -> OpenAICompatibleConfig {
    OpenAICompatibleConfig {
        name: "test-openai".to_string(),
        api_key: "test-key-123".to_string(),
        model: "gpt-4".to_string(),
        base_url: "http://localhost".to_string(),
        chat_api: "/v1/chat/completions".to_string(),
        temperature: Some(0.7),
        pricing_endpoint: None,
    }
}

fn create_backend_with_url(base_url: String) -> OpenAICompatibleBackend {
    let config = OpenAICompatibleConfig {
        base_url,
        ..create_test_config()
    };
    OpenAICompatibleBackend::new(config).unwrap()
}

#[tokio::test]
async fn backend_sends_simple_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("Authorization", "Bearer test-key-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "content": "Hi there!" },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5 }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let backend = create_backend_with_url(server.uri());
    let result = backend.send_message("Hello").await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Hi there!");
}

#[tokio::test]
async fn backend_handles_missing_api_key() {
    let config = OpenAICompatibleConfig {
        api_key: String::new(),
        ..create_test_config()
    };
    let backend = OpenAICompatibleBackend::new(config).unwrap();

    let result = backend.send_message("test").await;

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("API key not configured"));
}

#[tokio::test]
async fn backend_handles_network_error() {
    let backend =
        create_backend_with_url("http://invalid-host-that-does-not-exist-12345".to_string());

    let result = backend.send_message("test").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn backend_handles_rate_limit_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "60")
                .set_body_string("Rate limit exceeded"),
        )
        .expect(1..)
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let config = OpenAICompatibleConfig {
        base_url: server.uri(),
        ..create_test_config()
    };
    let default_executor = RequestExecutor::new(1, "Test".to_string());
    let backend = OpenAICompatibleBackend {
        client,
        config,
        default_executor,
        cached_pricing: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
    };

    let result = backend.send_message("test").await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Rate limit"));
}

#[tokio::test]
async fn backend_handles_server_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal server error"))
        .expect(1)
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let config = OpenAICompatibleConfig {
        base_url: server.uri(),
        ..create_test_config()
    };
    let default_executor = RequestExecutor::new(1, "Test".to_string());
    let backend = OpenAICompatibleBackend {
        client,
        config,
        default_executor,
        cached_pricing: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
    };

    let result = backend.send_message("test").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn backend_handles_authentication_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Invalid API key"))
        .expect(1)
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let config = OpenAICompatibleConfig {
        base_url: server.uri(),
        ..create_test_config()
    };
    let default_executor = RequestExecutor::new(1, "Test".to_string());
    let backend = OpenAICompatibleBackend {
        client,
        config,
        default_executor,
        cached_pricing: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
    };

    let result = backend.send_message("test").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn backend_sends_message_with_tools() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": {
                    "content": "I'll help you with that",
                    "tool_calls": [{
                        "id": "call_123",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\":\"San Francisco\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": { "prompt_tokens": 50, "completion_tokens": 25 }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let backend = create_backend_with_url(server.uri());
    let mut conversation = Conversation::new();
    conversation.add_user_message("What's the weather?".to_string());
    let tools = ToolRegistry::new();

    let result = backend.send_message_with_tools(&conversation, &tools).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.content.is_some());
    assert!(response.tool_calls.is_some());
    assert_eq!(response.tool_calls.as_ref().unwrap().len(), 1);
}

#[tokio::test]
async fn backend_handles_new_response_format() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "output": [{
                "content": [{
                    "type": "output_text",
                    "text": "Let me check that for you"
                }, {
                    "type": "tool_use",
                    "id": "tool_456",
                    "name": "search",
                    "input": {"query": "test"}
                }]
            }],
            "usage": { "input_tokens": 30, "output_tokens": 20 }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let backend = create_backend_with_url(server.uri());
    let mut conversation = Conversation::new();
    conversation.add_user_message("Search for something".to_string());
    let tools = ToolRegistry::new();

    let result = backend.send_message_with_tools(&conversation, &tools).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.content.is_some());
    assert!(response.tool_calls.is_some());
}

#[tokio::test]
async fn backend_tracks_token_usage() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "content": "Response" },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 100, "completion_tokens": 50 }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let backend = create_backend_with_url(server.uri());
    let mut conversation = Conversation::new();
    conversation.add_user_message("Test".to_string());
    let tools = ToolRegistry::new();

    let result = backend.send_message_with_tools(&conversation, &tools).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.input_tokens, Some(100));
    assert_eq!(response.output_tokens, Some(50));
}

#[tokio::test]
async fn backend_handles_response_truncated_by_length() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "content": "This response was cut off..." },
                "finish_reason": "length"
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 4096 }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let backend = create_backend_with_url(server.uri());
    let mut conversation = Conversation::new();
    conversation.add_user_message("Generate a very long response".to_string());
    let tools = ToolRegistry::new();

    let result = backend.send_message_with_tools(&conversation, &tools).await;

    assert!(result.is_err());
    if let Err(LlmError::RecoverableByLlm { message }) = result {
        assert!(message.contains("cut off"));
        assert!(message.contains("maximum token limit"));
    } else {
        panic!("Expected RecoverableByLlm error");
    }
}

#[tokio::test]
async fn backend_returns_correct_name_and_model() {
    let config = create_test_config();
    let backend = OpenAICompatibleBackend::new(config).unwrap();

    assert_eq!(backend.backend_name(), "test-openai");
    assert_eq!(backend.model_name(), "gpt-4");
}

#[tokio::test]
async fn backend_provides_pricing_for_known_models() {
    let mut config = create_test_config();
    config.model = "gpt-4o".to_string();
    let backend = OpenAICompatibleBackend::new(config).unwrap();

    let pricing = backend.pricing();
    assert!(pricing.is_some());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_per_million, 2.5);
    assert_eq!(pricing.output_per_million, 10.0);
}

#[tokio::test]
async fn backend_provides_pricing_for_gpt_4o_mini() {
    let mut config = create_test_config();
    config.model = "gpt-4o-mini".to_string();
    let backend = OpenAICompatibleBackend::new(config).unwrap();

    let pricing = backend.pricing();
    assert!(pricing.is_some());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_per_million, 0.15);
    assert_eq!(pricing.output_per_million, 0.6);
}

#[tokio::test]
async fn backend_returns_none_for_unknown_model_pricing() {
    let mut config = create_test_config();
    config.model = "unknown-model".to_string();
    let backend = OpenAICompatibleBackend::new(config).unwrap();

    let pricing = backend.pricing();
    assert!(pricing.is_none());
}

#[tokio::test]
async fn backend_handles_empty_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "choices": [] })))
        .expect(1)
        .mount(&server)
        .await;

    let backend = create_backend_with_url(server.uri());
    let result = backend.send_message("test").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn backend_handles_missing_content() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{ "message": {}, "finish_reason": "stop" }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let backend = create_backend_with_url(server.uri());
    let result = backend.send_message("test").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn backend_handles_multiple_retries_on_rate_limit() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "1")
                .set_body_string("Rate limit"),
        )
        .expect(2..)
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let config = OpenAICompatibleConfig {
        base_url: server.uri(),
        ..create_test_config()
    };
    let default_executor = RequestExecutor::new(2, "Test".to_string());
    let backend = OpenAICompatibleBackend {
        client,
        config,
        default_executor,
        cached_pricing: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
    };

    let result = backend.send_message("test").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn backend_uses_correct_headers() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("Authorization", "Bearer test-key-123"))
        .and(header_exists("content-type"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{ "message": { "content": "Success" } }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let backend = create_backend_with_url(server.uri());
    let _ = backend.send_message("test").await;
}

#[tokio::test]
async fn backend_sends_tools_in_request_when_available() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{ "message": { "content": "Response" } }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let backend = create_backend_with_url(server.uri());
    let mut conversation = Conversation::new();
    conversation.add_user_message("Test".to_string());
    let tools = ToolRegistry::new();

    let _ = backend.send_message_with_tools(&conversation, &tools).await;
}

#[tokio::test]
async fn backend_configuration_with_custom_values() {
    let config = OpenAICompatibleConfig {
        name: "custom".to_string(),
        api_key: "custom-key".to_string(),
        model: "custom-model".to_string(),
        base_url: "https://custom.api".to_string(),
        chat_api: "/custom/chat".to_string(),
        temperature: Some(0.9),
        pricing_endpoint: None,
    };

    let backend = OpenAICompatibleBackend::new(config).unwrap();

    assert_eq!(backend.backend_name(), "custom");
    assert_eq!(backend.model_name(), "custom-model");
}

#[tokio::test]
async fn backend_default_config_values() {
    let config = OpenAICompatibleConfig::default();

    assert_eq!(config.name, "openai");
    assert_eq!(config.model, "gpt-4");
    assert_eq!(config.base_url, "https://api.openai.com/v1");
    assert_eq!(config.chat_api, "/chat/completions");
}

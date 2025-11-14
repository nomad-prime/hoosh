use super::*;
use crate::agent::Conversation;
use crate::backends::{LlmBackend, LlmError};
use crate::tools::ToolRegistry;
use httpmock::prelude::*;
use serde_json::json;

fn create_test_config() -> OpenAICompatibleConfig {
    OpenAICompatibleConfig {
        name: "test-openai".to_string(),
        api_key: "test-key-123".to_string(),
        model: "gpt-4".to_string(),
        base_url: "http://localhost".to_string(),
        chat_api: "/v1/chat/completions".to_string(),
        temperature: Some(0.7),
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
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/chat/completions")
            .header("Authorization", "Bearer test-key-123")
            .json_body(json!({
                "model": "gpt-4",
                "messages": [{
                    "role": "user",
                    "content": "Hello"
                }],
                "max_completion_tokens": 4096,
                "temperature": 0.7
            }));
        then.status(200).json_body(json!({
            "choices": [{
                "message": {
                    "content": "Hi there!"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5
            }
        }));
    });

    let backend = create_backend_with_url(server.base_url());
    let result = backend.send_message("Hello").await;

    mock.assert();
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
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(429)
            .header("retry-after", "60")
            .body("Rate limit exceeded");
    });

    let config = OpenAICompatibleConfig {
        base_url: server.base_url(),
        ..create_test_config()
    };
    let client = reqwest::Client::new();
    let default_executor = RequestExecutor::new(1, "Test".to_string());
    let backend = OpenAICompatibleBackend {
        client,
        config,
        default_executor,
    };

    let result = backend.send_message("test").await;

    mock.assert();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("Rate limit"));
}

#[tokio::test]
async fn backend_handles_server_error() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(500).body("Internal server error");
    });

    let config = OpenAICompatibleConfig {
        base_url: server.base_url(),
        ..create_test_config()
    };
    let client = reqwest::Client::new();
    let default_executor = RequestExecutor::new(1, "Test".to_string());
    let backend = OpenAICompatibleBackend {
        client,
        config,
        default_executor,
    };

    let result = backend.send_message("test").await;

    mock.assert();
    assert!(result.is_err());
}

#[tokio::test]
async fn backend_handles_authentication_error() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(401).body("Invalid API key");
    });

    let config = OpenAICompatibleConfig {
        base_url: server.base_url(),
        ..create_test_config()
    };
    let client = reqwest::Client::new();
    let default_executor = RequestExecutor::new(1, "Test".to_string());
    let backend = OpenAICompatibleBackend {
        client,
        config,
        default_executor,
    };

    let result = backend.send_message("test").await;

    mock.assert();
    assert!(result.is_err());
}

#[tokio::test]
async fn backend_sends_message_with_tools() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200).json_body(json!({
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
            "usage": {
                "prompt_tokens": 50,
                "completion_tokens": 25
            }
        }));
    });

    let backend = create_backend_with_url(server.base_url());
    let mut conversation = Conversation::new();
    conversation.add_user_message("What's the weather?".to_string());
    let tools = ToolRegistry::new();

    let result = backend.send_message_with_tools(&conversation, &tools).await;

    mock.assert();
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.content.is_some());
    assert!(response.tool_calls.is_some());
    assert_eq!(response.tool_calls.as_ref().unwrap().len(), 1);
}

#[tokio::test]
async fn backend_handles_new_response_format() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200).json_body(json!({
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
            "usage": {
                "input_tokens": 30,
                "output_tokens": 20
            }
        }));
    });

    let backend = create_backend_with_url(server.base_url());
    let mut conversation = Conversation::new();
    conversation.add_user_message("Search for something".to_string());
    let tools = ToolRegistry::new();

    let result = backend.send_message_with_tools(&conversation, &tools).await;

    mock.assert();
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.content.is_some());
    assert!(response.tool_calls.is_some());
}

#[tokio::test]
async fn backend_tracks_token_usage() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200).json_body(json!({
            "choices": [{
                "message": {
                    "content": "Response"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50
            }
        }));
    });

    let backend = create_backend_with_url(server.base_url());
    let mut conversation = Conversation::new();
    conversation.add_user_message("Test".to_string());
    let tools = ToolRegistry::new();

    let result = backend.send_message_with_tools(&conversation, &tools).await;

    mock.assert();
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.input_tokens, Some(100));
    assert_eq!(response.output_tokens, Some(50));
}

#[tokio::test]
async fn backend_handles_response_truncated_by_length() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200).json_body(json!({
            "choices": [{
                "message": {
                    "content": "This response was cut off..."
                },
                "finish_reason": "length"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 4096
            }
        }));
    });

    let backend = create_backend_with_url(server.base_url());
    let mut conversation = Conversation::new();
    conversation.add_user_message("Generate a very long response".to_string());
    let tools = ToolRegistry::new();

    let result = backend.send_message_with_tools(&conversation, &tools).await;

    mock.assert();
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
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200).json_body(json!({
            "choices": []
        }));
    });

    let backend = create_backend_with_url(server.base_url());
    let result = backend.send_message("test").await;

    mock.assert();
    assert!(result.is_err());
}

#[tokio::test]
async fn backend_handles_missing_content() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200).json_body(json!({
            "choices": [{
                "message": {},
                "finish_reason": "stop"
            }]
        }));
    });

    let backend = create_backend_with_url(server.base_url());
    let result = backend.send_message("test").await;

    mock.assert();
    assert!(result.is_err());
}

#[tokio::test]
async fn backend_handles_multiple_retries_on_rate_limit() {
    let server = MockServer::start();

    let fail_mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(429)
            .header("retry-after", "1")
            .body("Rate limit");
    });

    let config = OpenAICompatibleConfig {
        base_url: server.base_url(),
        ..create_test_config()
    };
    let client = reqwest::Client::new();
    let default_executor = RequestExecutor::new(2, "Test".to_string());
    let backend = OpenAICompatibleBackend {
        client,
        config,
        default_executor,
    };

    let result = backend.send_message("test").await;

    assert!(fail_mock.hits() >= 2);
    assert!(result.is_err());
}

#[tokio::test]
async fn backend_uses_correct_headers() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/chat/completions")
            .header("Authorization", "Bearer test-key-123")
            .header_exists("content-type");
        then.status(200).json_body(json!({
            "choices": [{
                "message": {
                    "content": "Success"
                }
            }]
        }));
    });

    let backend = create_backend_with_url(server.base_url());
    let _ = backend.send_message("test").await;

    mock.assert();
}

#[tokio::test]
async fn backend_sends_tools_in_request_when_available() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200).json_body(json!({
            "choices": [{
                "message": {
                    "content": "Response"
                }
            }]
        }));
    });

    let backend = create_backend_with_url(server.base_url());
    let mut conversation = Conversation::new();
    conversation.add_user_message("Test".to_string());

    let tools = ToolRegistry::new();

    let _ = backend.send_message_with_tools(&conversation, &tools).await;

    mock.assert();
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

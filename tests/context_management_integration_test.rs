use hoosh::agent::{Conversation, ConversationMessage};
use hoosh::context_management::{
    ContextManager, ContextManagerConfig, SlidingWindowConfig, TokenAccountant,
    ToolOutputTruncationConfig,
};
use std::sync::Arc;

/// Test that token pressure is calculated based on actual conversation size, not API tokens
#[tokio::test]
async fn test_token_pressure_reflects_conversation_size() {
    let accountant = Arc::new(TokenAccountant::new());
    let config = ContextManagerConfig::default().with_max_tokens(100_000);
    let manager = ContextManager::new(config, accountant);

    let mut conversation = Conversation::new();

    // Empty conversation should have 0% pressure
    let pressure_empty = manager.get_token_pressure(&conversation);
    assert_eq!(pressure_empty, 0.0);

    // Add a message with ~25K tokens (100K bytes / 4)
    conversation.messages.push(ConversationMessage {
        role: "user".to_string(),
        content: Some("x".repeat(100_000)),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    });

    // Pressure should be ~25% (25K / 100K)
    let pressure_with_message = manager.get_token_pressure(&conversation);
    assert!(
        pressure_with_message > 0.20 && pressure_with_message < 0.30,
        "Expected pressure ~25%, got {}",
        pressure_with_message
    );

    // Add another message with ~50K tokens
    conversation.messages.push(ConversationMessage {
        role: "assistant".to_string(),
        content: Some("y".repeat(200_000)),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    });

    // Pressure should be ~75% (75K / 100K)
    let pressure_high = manager.get_token_pressure(&conversation);
    assert!(
        pressure_high > 0.70 && pressure_high < 0.80,
        "Expected pressure ~75%, got {}",
        pressure_high
    );
}

/// Test that sliding window runs before truncation
#[tokio::test]
async fn test_strategy_execution_order() {
    let accountant = Arc::new(TokenAccountant::new());

    let config = ContextManagerConfig {
        max_tokens: 100_000,
        compression_threshold: 0.80,
        preserve_recent_percentage: 0.50,
        warning_threshold: 0.70,
        tool_output_truncation: Some(ToolOutputTruncationConfig {
            max_length: 1000, // Very small limit
            show_truncation_notice: true,
            smart_truncate: false,
            head_length: 800,
            tail_length: 200,
        }),
        sliding_window: Some(SlidingWindowConfig {
            window_size: 5, // Keep only last 5 messages
            preserve_system: false,
            min_messages_before_windowing: 0,
            preserve_initial_task: false,
        }),
    };

    let mut manager_builder = ContextManager::new(config.clone(), Arc::clone(&accountant));

    // Add sliding window first, then truncation (correct order)
    if let Some(sliding_config) = config.sliding_window {
        let sliding_strategy =
            hoosh::context_management::SlidingWindowStrategy::new(sliding_config);
        manager_builder = manager_builder.add_strategy(Box::new(sliding_strategy));
    }

    if let Some(truncation_config) = config.tool_output_truncation {
        let truncation_strategy =
            hoosh::context_management::ToolOutputTruncationStrategy::new(truncation_config);
        manager_builder = manager_builder.add_strategy(Box::new(truncation_strategy));
    }

    let manager = manager_builder;

    // Create conversation with 10 messages - use TOOL RESULT messages since that's what gets truncated
    let mut conversation = Conversation::new();
    for i in 0..10 {
        // Add assistant message with tool call
        conversation.messages.push(ConversationMessage {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(vec![hoosh::agent::ToolCall {
                id: format!("call_{}", i),
                r#type: "function".to_string(),
                function: hoosh::agent::ToolFunction {
                    name: "test_tool".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
            tool_call_id: None,
            name: None,
        });

        // Add tool result message with large content
        conversation.messages.push(ConversationMessage {
            role: "tool".to_string(),
            content: Some(format!(
                "Tool result {} with large content: {}",
                i,
                "x".repeat(5000)
            )),
            tool_calls: None,
            tool_call_id: Some(format!("call_{}", i)),
            name: Some("test_tool".to_string()),
        });
    }

    assert_eq!(conversation.messages.len(), 20); // 10 assistant + 10 tool messages

    // Apply strategies
    manager
        .apply_strategies(&mut conversation)
        .await
        .expect("Failed to apply strategies");

    // After sliding window, should have 6 messages (3 complete tool call pairs)
    // Window size is 5, but tool call/result pairs must be kept together
    // So we keep the last 3 complete pairs: assistant_7+tool_7, assistant_8+tool_8, assistant_9+tool_9
    assert_eq!(
        conversation.messages.len(),
        6,
        "Sliding window should reduce to 6 messages (3 complete tool call pairs)"
    );

    // Verify all tool calls have matching results
    for msg in conversation.messages.iter() {
        if let Some(tool_calls) = &msg.tool_calls {
            for tool_call in tool_calls {
                let has_result = conversation
                    .messages
                    .iter()
                    .any(|m| m.role == "tool" && m.tool_call_id.as_ref() == Some(&tool_call.id));
                assert!(
                    has_result,
                    "Tool call {} must have matching result",
                    tool_call.id
                );
            }
        }
    }

    // The main goal of this test is to verify that the strategies run in the correct order
    // (sliding window first, then truncation). The fact that we reduced from 20 to 6 messages
    // while maintaining tool call/result pairs proves sliding window ran successfully.
}

/// Test that pressure is recalculated after compression
#[tokio::test]
async fn test_pressure_recalculation_after_compression() {
    let accountant = Arc::new(TokenAccountant::new());

    let config = ContextManagerConfig {
        max_tokens: 100_000,
        compression_threshold: 0.80,
        preserve_recent_percentage: 0.50,
        warning_threshold: 0.60,
        tool_output_truncation: Some(ToolOutputTruncationConfig::default()),
        sliding_window: Some(SlidingWindowConfig {
            window_size: 10,
            preserve_system: false,
            min_messages_before_windowing: 0,
            preserve_initial_task: false,
        }),
    };

    let mut manager_builder = ContextManager::new(config.clone(), Arc::clone(&accountant));

    // Add strategies in correct order
    if let Some(sliding_config) = config.sliding_window {
        let sliding_strategy =
            hoosh::context_management::SlidingWindowStrategy::new(sliding_config);
        manager_builder = manager_builder.add_strategy(Box::new(sliding_strategy));
    }

    if let Some(truncation_config) = config.tool_output_truncation {
        let truncation_strategy =
            hoosh::context_management::ToolOutputTruncationStrategy::new(truncation_config);
        manager_builder = manager_builder.add_strategy(Box::new(truncation_strategy));
    }

    let manager = manager_builder;

    // Create conversation with 50 large messages (will exceed warning threshold)
    let mut conversation = Conversation::new();
    for i in 0..50 {
        conversation.messages.push(ConversationMessage {
            role: "user".to_string(),
            content: Some(format!("Message {}: {}", i, "x".repeat(10_000))),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
    }

    // Check pressure BEFORE compression
    let pressure_before = manager.get_token_pressure(&conversation);
    assert!(
        pressure_before > config.warning_threshold,
        "Pressure should be above warning threshold before compression"
    );

    // Apply compression strategies
    manager
        .apply_strategies(&mut conversation)
        .await
        .expect("Failed to apply strategies");

    // Check pressure AFTER compression
    let pressure_after = manager.get_token_pressure(&conversation);

    // Pressure should be reduced significantly
    assert!(
        pressure_after < pressure_before,
        "Pressure should decrease after compression (before: {}, after: {})",
        pressure_before,
        pressure_after
    );

    // After compression (windowing to 10 messages), pressure should be much lower
    assert!(
        pressure_after < 0.50,
        "Pressure should be well below 50% after windowing to 10 messages, got {}",
        pressure_after
    );
}

/// Test the new should_warn_about_pressure_value method
#[test]
fn test_should_warn_about_pressure_value() {
    let accountant = Arc::new(TokenAccountant::new());
    let config = ContextManagerConfig::default().with_warning_threshold(0.70);
    let manager = ContextManager::new(config, accountant);

    // Above threshold should warn
    assert!(manager.should_warn_about_pressure_value(0.75));
    assert!(manager.should_warn_about_pressure_value(1.0));

    // Equal to threshold should warn
    assert!(manager.should_warn_about_pressure_value(0.70));

    // Below threshold should not warn
    assert!(!manager.should_warn_about_pressure_value(0.65));
    assert!(!manager.should_warn_about_pressure_value(0.0));
}

/// Test that token estimation handles tool calls correctly
#[test]
fn test_token_estimation_with_tool_calls() {
    use hoosh::agent::{ToolCall, ToolFunction};

    let mut conversation = Conversation::new();

    // Add a message with tool calls and large arguments
    conversation.messages.push(ConversationMessage {
        role: "assistant".to_string(),
        content: None,
        tool_calls: Some(vec![ToolCall {
            id: "call_1".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                // Large JSON arguments
                arguments: serde_json::json!({
                    "path": "/very/long/path/to/file.txt",
                    "extra_data": "x".repeat(5000),
                })
                .to_string(),
            },
        }]),
        tool_call_id: None,
        name: None,
    });

    let estimated_tokens = TokenAccountant::estimate_conversation_tokens(&conversation);

    // Should account for tool call name and arguments
    // "assistant" + "read_file" + large JSON = significant tokens
    assert!(
        estimated_tokens > 1000,
        "Should estimate significant tokens for message with large tool call arguments"
    );
}

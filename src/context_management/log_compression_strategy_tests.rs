use super::*;
use crate::agent::{Conversation, ConversationMessage, Role};
use crate::context_management::LogCompressionConfig;
use crate::tools::{BuiltinToolProvider, ToolRegistry};
use std::path::PathBuf;
use std::sync::Arc;

fn compressor() -> LogCompressor {
    LogCompressor::new(LogCompressionConfig::default())
}

fn strategy() -> LogCompressionStrategy {
    let registry =
        ToolRegistry::new().with_provider(Arc::new(BuiltinToolProvider::new(PathBuf::from("."))));
    LogCompressionStrategy::new(LogCompressionConfig::default(), Arc::new(registry))
}

fn tool_result(content: &str) -> ConversationMessage {
    tool_result_named("bash", content)
}

fn tool_result_named(name: &str, content: &str) -> ConversationMessage {
    ConversationMessage {
        role: Role::Tool,
        content: Some(content.to_string()),
        tool_calls: None,
        tool_call_id: Some("call_1".to_string()),
        name: Some(name.to_string()),
        attachments: Vec::new(),
    }
}

#[test]
fn leaves_short_output_untouched() {
    let content = (0..10)
        .map(|i| format!("INFO line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(compressor().compress(&content), None);
}

#[test]
fn skips_generic_text_with_no_log_signal() {
    let content = (0..80)
        .map(|i| format!("the quick brown fox number {i} jumps"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(compressor().compress(&content), None);
}

#[test]
fn keeps_error_buried_in_noise() {
    let mut lines: Vec<String> = (0..200).map(|i| format!("INFO step {i} ok")).collect();
    lines.insert(120, "ERROR database connection refused".to_string());
    let content = lines.join("\n");

    let compressed = compressor().compress(&content).expect("should compress");
    assert!(compressed.contains("ERROR database connection refused"));
    assert!(compressed.lines().count() < 200);
    assert!(compressed.contains("lines omitted"));
}

#[test]
fn level_word_boundary_does_not_overfire() {
    let mut lines: Vec<String> = (0..80).map(|i| format!("processing item {i}")).collect();
    lines.push("errorless run, informant cleared, warned-off nobody".to_string());
    let content = lines.join("\n");
    assert_eq!(
        compressor().compress(&content),
        None,
        "substring matches must not register as a real ERROR/WARN signal"
    );
}

#[test]
fn preserves_python_traceback_across_blank_lines() {
    let mut lines: Vec<String> = (0..80).map(|i| format!("INFO setup {i}")).collect();
    lines.extend(
        [
            "Traceback (most recent call last):",
            "  File \"a.py\", line 1, in <module>",
            "ValueError: bad value",
            "",
            "During handling of the above exception, another exception occurred:",
            "",
            "Traceback (most recent call last):",
            "  File \"b.py\", line 2, in <module>",
            "RuntimeError: downstream failure",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    let content = lines.join("\n");

    let compressed = compressor().compress(&content).expect("should compress");
    assert!(compressed.contains("ValueError: bad value"));
    assert!(compressed.contains("RuntimeError: downstream failure"));
    assert!(compressed.contains("File \"b.py\", line 2"));
}

#[test]
fn dedupes_near_identical_warnings_spaced_apart() {
    let mut lines: Vec<String> = (0..60)
        .map(|i| format!("INFO compiling unit {i}"))
        .collect();
    lines.insert(
        10,
        "warning: unused variable in /tmp/a/123 module".to_string(),
    );
    lines.insert(
        40,
        "warning: unused variable in /tmp/b/999 module".to_string(),
    );
    let content = lines.join("\n");

    let compressed = compressor().compress(&content).expect("should compress");
    let warning_lines = compressed
        .lines()
        .filter(|l| l.starts_with("warning: unused variable"))
        .count();
    assert_eq!(warning_lines, 1, "near-identical warnings collapse to one");
}

#[tokio::test]
async fn strategy_compresses_tool_results_in_place() {
    let mut lines: Vec<String> = (0..200).map(|i| format!("INFO step {i} ok")).collect();
    lines.insert(90, "ERROR fatal crash in handler".to_string());
    let big_log = lines.join("\n");

    let mut conversation = Conversation::new();
    conversation.messages.push(ConversationMessage {
        role: Role::User,
        content: Some("run the tests".to_string()),
        tool_calls: None,
        tool_call_id: None,
        name: None,
        attachments: Vec::new(),
    });
    conversation.messages.push(tool_result(&big_log));

    let strategy = strategy();
    let result = strategy.apply(&mut conversation).await.unwrap();

    assert_eq!(result, StrategyResult::Applied);
    let compressed = conversation.messages[1].content.as_ref().unwrap();
    assert!(compressed.len() < big_log.len());
    assert!(compressed.contains("ERROR fatal crash in handler"));
}

#[tokio::test]
async fn strategy_preserves_read_file_output() {
    let mut lines: Vec<String> = (0..200).map(|i| format!("INFO step {i} ok")).collect();
    lines.insert(90, "ERROR fatal crash in handler".to_string());
    let file = lines.join("\n");

    let mut conversation = Conversation::new();
    conversation
        .messages
        .push(tool_result_named("read_file", &file));

    let strategy = strategy();
    let result = strategy.apply(&mut conversation).await.unwrap();

    assert_eq!(result, StrategyResult::NoChange);
    assert_eq!(conversation.messages[0].content.as_ref().unwrap(), &file);
}

#[tokio::test]
async fn strategy_ignores_non_tool_messages() {
    let big_text = (0..200)
        .map(|i| format!("ERROR noise {i}"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut conversation = Conversation::new();
    conversation.messages.push(ConversationMessage {
        role: Role::User,
        content: Some(big_text.clone()),
        tool_calls: None,
        tool_call_id: None,
        name: None,
        attachments: Vec::new(),
    });

    let strategy = strategy();
    let result = strategy.apply(&mut conversation).await.unwrap();

    assert_eq!(result, StrategyResult::NoChange);
    assert_eq!(
        conversation.messages[0].content.as_ref().unwrap(),
        &big_text
    );
}

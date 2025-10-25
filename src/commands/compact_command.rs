use anyhow::Result;
use async_trait::async_trait;

use crate::Command;
use crate::CommandContext;
use crate::CommandResult;
use crate::conversations::AgentEvent;

pub struct CompactCommand;

#[async_trait]
impl Command for CompactCommand {
    fn name(&self) -> &str {
        "compact"
    }

    fn description(&self) -> &str {
        "Compress old conversation history to save context"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["summarize", "compress"]
    }

    fn usage(&self) -> &str {
        "/compact [keep_recent] - Summarize old messages, keeping N recent (default: 15)"
    }

    async fn execute(
        &self,
        args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        let keep_recent = args
            .first()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(15);

        // Get the conversation from context
        let conversation = context
            .conversation
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No conversation available"))?;

        let messages_len = {
            let conv = conversation.lock().await;
            conv.get_messages_for_api().len()
        };

        if messages_len < 30 {
            return Ok(CommandResult::Success(
                "Conversation too short to compact (< 30 messages)".to_string(),
            ));
        }

        // Get the summarizer from context
        let summarizer = context
            .summarizer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No summarizer available"))?;

        // Get messages and create summary
        let summary = {
            let conv = conversation.lock().await;
            let messages = conv.get_messages_for_api();
            let old_messages = &messages[..messages.len() - keep_recent];

            // Send summarizing event
            if let Some(event_tx) = &context.event_tx {
                let _ = event_tx.send(AgentEvent::Summarizing {
                    message_count: old_messages.len(),
                });
            }

            // Use MessageSummarizer from context
            let result = summarizer.summarize(old_messages, None).await;

            // Send appropriate event based on result
            if let Some(event_tx) = &context.event_tx {
                match &result {
                    Ok(summary) => {
                        let _ = event_tx.send(AgentEvent::SummaryComplete {
                            message_count: old_messages.len(),
                            summary: summary.clone(),
                        });
                    }
                    Err(e) => {
                        let _ = event_tx.send(AgentEvent::SummaryError {
                            error: e.to_string(),
                        });
                    }
                }
            }

            result?
        };

        // Replace in conversation
        {
            let mut conv = conversation.lock().await;
            conv.compact_with_summary(summary, keep_recent);
        }

        Ok(CommandResult::Success(format!(
            "Compacted {} messages into summary",
            messages_len - keep_recent
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::mock::MockBackend;
    use crate::conversations::Conversation;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_compact_command_creation() {
        let command = CompactCommand;
        assert_eq!(command.name(), "compact");
        assert_eq!(
            command.description(),
            "Compress old conversation history to save context"
        );
        assert!(command.aliases().contains(&"summarize"));
        assert!(command.aliases().contains(&"compress"));
    }

    #[tokio::test]
    async fn test_compact_command_short_conversation() {
        let mut context = CommandContext::new();

        // Create a conversation with only 10 messages (too short to compact)
        let conversation = Arc::new(Mutex::new(Conversation::new()));
        {
            let mut conv = conversation.lock().await;
            for i in 0..10 {
                conv.add_user_message(format!("Message {}", i));
            }
        }

        context.conversation = Some(conversation);

        // Create a mock backend
        let backend = Arc::new(MockBackend::new());
        context.backend = Some(backend);

        let command = CompactCommand;
        let result = command.execute(vec![], &mut context).await.unwrap();

        match result {
            CommandResult::Success(msg) => {
                assert_eq!(msg, "Conversation too short to compact (< 30 messages)");
            }
            _ => panic!("Expected Success result"),
        }
    }
}

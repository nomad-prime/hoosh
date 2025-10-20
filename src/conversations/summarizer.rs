use anyhow::{Context, Result};
use std::sync::Arc;

use crate::backends::LlmBackend;
use crate::conversations::ConversationMessage;

/// MessageSummarizer handles LLM-based summarization of conversation messages
#[derive(Clone)]
pub struct MessageSummarizer {
    backend: Arc<dyn LlmBackend>,
}

impl MessageSummarizer {
    /// Create a new MessageSummarizer with the specified LLM backend
    pub fn new(backend: Arc<dyn LlmBackend>) -> Self {
        Self { backend }
    }

    pub async fn summarize(
        &self,
        messages: &[ConversationMessage],
        focus_areas: Option<Vec<String>>,
    ) -> Result<String> {
        // Build the summary request prompt
        let summary_request = ConversationMessage {
            role: "user".to_string(),
            content: Some(self.build_summary_request(focus_areas)),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };

        // Create context with messages to summarize + summary request
        let mut context_messages = messages.to_vec();
        context_messages.push(summary_request);

        // Make API call to summarize the conversation
        let response = self
            .backend
            .send_message(&self.format_messages_for_summary(&context_messages))
            .await
            .context("Failed to get summary from backend")?;

        Ok(response)
    }

    fn build_summary_request(&self, focus_areas: Option<Vec<String>>) -> String {
        let mut request = String::from(
            "Summarize our conversation so far concisely. Focus on:\n\
             - Key decisions, configurations, and code changes\n\
             - Important context needed for future reference\n\
             - Unresolved issues or pending tasks\n\
             - Critical file paths, functions, or entities mentioned\n\n",
        );

        if let Some(areas) = focus_areas {
            request.push_str(&format!(
                "Pay special attention to: {}\n\n",
                areas.join(", ")
            ));
        }

        request.push_str(
            "Omit routine acknowledgments and redundant information.\n\
             Aim for 70% compression while preserving semantic value.\n\
             Provide only the summary, no preamble.",
        );

        request
    }

    /// Format messages for the summary request
    fn format_messages_for_summary(&self, messages: &[ConversationMessage]) -> String {
        let mut formatted = String::new();

        for message in messages {
            formatted.push_str(&format!(
                "{}: {}\n",
                message.role,
                message.content.as_deref().unwrap_or("")
            ));
        }

        formatted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::mock::MockBackend;

    #[tokio::test]
    async fn test_summarizer_creation() {
        let mock_backend = Arc::new(MockBackend::new());
        let _summarizer = MessageSummarizer::new(mock_backend);
        assert!(true); // Just testing creation works
    }

    #[tokio::test]
    async fn test_build_summary_request_without_focus_areas() {
        let mock_backend = Arc::new(MockBackend::new());
        let summarizer = MessageSummarizer::new(mock_backend);

        let request = summarizer.build_summary_request(None);
        assert!(request.contains("Summarize our conversation so far concisely"));
        assert!(request.contains("Key decisions, configurations, and code changes"));
    }

    #[tokio::test]
    async fn test_build_summary_request_with_focus_areas() {
        let mock_backend = Arc::new(MockBackend::new());
        let summarizer = MessageSummarizer::new(mock_backend);

        let focus_areas = vec!["testing".to_string(), "performance".to_string()];
        let request = summarizer.build_summary_request(Some(focus_areas));
        assert!(request.contains("Pay special attention to: testing, performance"));
    }
}

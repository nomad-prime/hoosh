pub mod tool;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MemoryMode {
    #[default]
    Conversation,
    Summary,
}

impl MemoryMode {
    pub const VARIANTS: &'static [&'static str] = &["conversation", "summary"];
}

impl FromStr for MemoryMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "conversation" => Ok(Self::Conversation),
            "summary" => Ok(Self::Summary),
            _ => Err(anyhow!("Invalid memory mode: {}", s)),
        }
    }
}

impl std::fmt::Display for MemoryMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conversation => write!(f, "conversation"),
            Self::Summary => write!(f, "summary"),
        }
    }
}

pub const SUMMARY_MODE_AGENT_INSTRUCTIONS: &str =
    include_str!("../prompts/memory_summary_instructions.txt");

pub struct MemoryModeManager {
    conversation_id: String,
    memory_dir: PathBuf,
    last_turn_start: Arc<Mutex<Option<SystemTime>>>,
    pub instructions: String,
}

impl MemoryModeManager {
    pub fn new(conversation_id: &str) -> Result<Self> {
        let memory_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".hoosh")
            .join("memory")
            .join(conversation_id);
        fs::create_dir_all(&memory_dir)?;
        let instructions = crate::config::AppConfig::load_memory_summary_instructions();
        Ok(Self {
            conversation_id: conversation_id.to_string(),
            memory_dir,
            last_turn_start: Arc::new(Mutex::new(None)),
            instructions,
        })
    }

    pub fn conversation_id(&self) -> &str {
        &self.conversation_id
    }

    pub fn summary_path(&self) -> PathBuf {
        self.memory_dir.join("summary.txt")
    }

    pub fn read_summary(&self) -> Option<String> {
        fs::read_to_string(self.summary_path()).ok()
    }

    pub fn summary_written_since_last_turn(&self) -> bool {
        let path = self.summary_path();
        if !path.exists() {
            return false;
        }

        let guard = match self.last_turn_start.lock() {
            Ok(g) => g,
            Err(_) => return false,
        };
        let last_turn = match *guard {
            Some(t) => t,
            None => return false,
        };

        let mtime = match fs::metadata(&path).and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => return false,
        };

        mtime > last_turn
    }

    pub fn record_turn_end(&self, turn_start: SystemTime) {
        if let Ok(mut guard) = self.last_turn_start.lock() {
            *guard = Some(turn_start);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Conversation, Role};
    use crate::memory_mode::tool::UpdateSessionFileTool;
    use crate::tools::Tool;
    use serde_json::json;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Guard for tests that call set_current_dir (process-wide state)
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_memory_mode_defaults_to_conversation() {
        assert_eq!(MemoryMode::default(), MemoryMode::Conversation);
    }

    #[test]
    fn test_memory_mode_conversation_is_default() {
        let mode: MemoryMode = Default::default();
        assert_eq!(mode, MemoryMode::Conversation);
    }

    #[test]
    fn test_memory_mode_parses_summary_from_str() {
        assert_eq!(
            "summary".parse::<MemoryMode>().unwrap(),
            MemoryMode::Summary
        );
    }

    #[test]
    fn test_memory_mode_parses_conversation_from_str() {
        assert_eq!(
            "conversation".parse::<MemoryMode>().unwrap(),
            MemoryMode::Conversation
        );
    }

    #[test]
    fn test_memory_mode_invalid_str_errors() {
        assert!("invalid".parse::<MemoryMode>().is_err());
    }

    #[test]
    fn test_memory_mode_serializes_lowercase() {
        let summary = MemoryMode::Summary;
        let json = serde_json::to_string(&summary).unwrap();
        assert_eq!(json, r#""summary""#);

        let conv = MemoryMode::Conversation;
        let json = serde_json::to_string(&conv).unwrap();
        assert_eq!(json, r#""conversation""#);
    }

    #[test]
    fn test_injection_skipped_silently_on_first_turn() {
        let dir = TempDir::new().unwrap();
        let manager = MemoryModeManager {
            conversation_id: "test".to_string(),
            memory_dir: dir.path().to_path_buf(),
            last_turn_start: Arc::new(Mutex::new(None)),
            instructions: SUMMARY_MODE_AGENT_INSTRUCTIONS.to_string(),
        };
        assert!(!manager.summary_written_since_last_turn());
    }

    fn make_tool_context(conv_id: &str) -> crate::tools::ToolExecutionContext {
        crate::tools::ToolExecutionContext {
            tool_call_id: "test-call-id".to_string(),
            event_tx: None,
            parent_conversation_id: Some(conv_id.to_string()),
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_tool_writes_summary_to_correct_path() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let tool = UpdateSessionFileTool;
        let context = make_tool_context("conv-abc123");
        let result = tool
            .execute(&json!({ "summary": "Test summary content" }), &context)
            .await;
        assert!(result.is_ok());

        let expected = dir
            .path()
            .join(".hoosh")
            .join("memory")
            .join("conv-abc123")
            .join("summary.txt");
        assert!(expected.exists());
        assert_eq!(
            fs::read_to_string(&expected).unwrap(),
            "Test summary content"
        );

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_tool_overwrites_existing_summary() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let tool = UpdateSessionFileTool;
        let context = make_tool_context("conv-overwrite");

        let _ = tool
            .execute(&json!({ "summary": "first summary" }), &context)
            .await;
        let _ = tool
            .execute(&json!({ "summary": "second summary" }), &context)
            .await;

        let path = dir
            .path()
            .join(".hoosh")
            .join("memory")
            .join("conv-overwrite")
            .join("summary.txt");
        let content = fs::read_to_string(&path).unwrap();

        std::env::set_current_dir(original_dir).unwrap();
        assert_eq!(content, "second summary");
    }

    #[test]
    fn test_manager_creates_directory_on_new() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let conv_id = "test-conv-123";
        let expected_path = dir.path().join(".hoosh").join("memory").join(conv_id);

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let manager = MemoryModeManager::new(conv_id).unwrap();
        assert!(manager.memory_dir.exists());

        std::env::set_current_dir(original_dir).unwrap();
        assert!(expected_path.exists());
    }

    #[test]
    fn test_read_summary_returns_none_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let manager = MemoryModeManager {
            conversation_id: "test".to_string(),
            memory_dir: dir.path().to_path_buf(),
            last_turn_start: Arc::new(Mutex::new(None)),
            instructions: SUMMARY_MODE_AGENT_INSTRUCTIONS.to_string(),
        };
        assert!(manager.read_summary().is_none());
    }

    #[test]
    fn test_read_summary_returns_content_when_present() {
        let dir = TempDir::new().unwrap();
        let manager = MemoryModeManager {
            conversation_id: "test".to_string(),
            memory_dir: dir.path().to_path_buf(),
            last_turn_start: Arc::new(Mutex::new(None)),
            instructions: SUMMARY_MODE_AGENT_INSTRUCTIONS.to_string(),
        };
        fs::write(manager.summary_path(), "hello world").unwrap();
        assert_eq!(manager.read_summary().unwrap(), "hello world");
    }

    #[test]
    fn test_summary_modified_since_returns_false_when_missing() {
        let dir = TempDir::new().unwrap();
        let manager = MemoryModeManager {
            conversation_id: "test".to_string(),
            memory_dir: dir.path().to_path_buf(),
            last_turn_start: Arc::new(Mutex::new(Some(SystemTime::UNIX_EPOCH))),
            instructions: SUMMARY_MODE_AGENT_INSTRUCTIONS.to_string(),
        };
        assert!(!manager.summary_written_since_last_turn());
    }

    #[test]
    fn test_summary_modified_since_detects_write() {
        let dir = TempDir::new().unwrap();
        let turn_start = SystemTime::now();
        std::thread::sleep(std::time::Duration::from_millis(10));

        let manager = MemoryModeManager {
            conversation_id: "test".to_string(),
            memory_dir: dir.path().to_path_buf(),
            last_turn_start: Arc::new(Mutex::new(Some(turn_start))),
            instructions: SUMMARY_MODE_AGENT_INSTRUCTIONS.to_string(),
        };

        fs::write(manager.summary_path(), "summary content").unwrap();
        assert!(manager.summary_written_since_last_turn());
    }

    #[test]
    fn test_integration_full_turn_cycle() {
        let dir = TempDir::new().unwrap();
        let conv_id = "integration-test-conv";

        // Build manager pointing at a known temp directory (no set_current_dir needed)
        let memory_dir = dir.path().join(".hoosh").join("memory").join(conv_id);
        fs::create_dir_all(&memory_dir).unwrap();
        let manager = MemoryModeManager {
            conversation_id: conv_id.to_string(),
            memory_dir: memory_dir.clone(),
            last_turn_start: Arc::new(Mutex::new(None)),
            instructions: SUMMARY_MODE_AGENT_INSTRUCTIONS.to_string(),
        };

        // Turn 1 start — no prior summary
        assert!(!manager.summary_written_since_last_turn());
        assert!(manager.read_summary().is_none());

        // Simulate agent writing summary at end of turn 1
        let summary_content = "**Goal**: Test memory mode.\n**This turn**: Verified injection.\n**State**: Working.\n**Next**: Done.";
        fs::write(manager.summary_path(), summary_content).unwrap();

        // Record turn end with a past timestamp so mtime appears newer
        let past = SystemTime::UNIX_EPOCH; // file was written after epoch, so mtime > past
        manager.record_turn_end(past);

        // Turn 2: manager detects the written summary
        assert!(manager.summary_written_since_last_turn());

        // Simulate injection: fresh conversation with 2 system messages + prior turn content
        let mut conv = Conversation::new();
        conv.add_system_message("agent definition".to_string());
        conv.add_system_message("env context".to_string());
        conv.add_user_message("first user message".to_string());
        conv.add_assistant_message(Some("first response".to_string()), None);

        // answer() clears turn history (since summary was written)
        assert_eq!(conv.messages.len(), 4);
        conv.clear_turn_history();
        assert_eq!(conv.messages.len(), 2);

        // Inject instructions + summary
        let summary = manager.read_summary().unwrap();
        let content = format!(
            "{}\n\n## Session Memory\n\n{}",
            SUMMARY_MODE_AGENT_INSTRUCTIONS, summary
        );
        conv.add_system_message(content.clone());

        assert!(content.contains("update_session_file"));
        assert!(content.contains("Session Memory"));
        assert!(content.contains("Test memory mode"));

        // Turn 2 user message
        conv.add_user_message("second user message".to_string());

        // Final: 2 original system msgs + 1 injected + 1 user
        assert_eq!(conv.messages.len(), 4);
        assert_eq!(conv.messages[0].role, Role::System);
        assert_eq!(conv.messages[1].role, Role::System);
        assert_eq!(conv.messages[2].role, Role::System);
        assert_eq!(conv.messages[3].role, Role::User);
    }
}

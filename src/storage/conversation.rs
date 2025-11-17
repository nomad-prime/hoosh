use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::IndexStorage;
use crate::agent::ConversationMessage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMetadata {
    pub id: String,
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub message_count: usize,
}

impl ConversationMetadata {
    pub fn new(id: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id,
            title: String::new(),
            created_at: now,
            updated_at: now,
            message_count: 0,
        }
    }

    pub fn with_title(mut self, title: String) -> Self {
        self.title = title;
        self
    }

    pub fn update(&mut self) {
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

pub struct ConversationStorage {
    base_path: PathBuf,
    index: IndexStorage,
}

impl ConversationStorage {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let index = IndexStorage::with_default_path()?;
        Ok(Self {
            base_path: base_path.as_ref().to_path_buf(),
            index,
        })
    }

    pub fn new_with_index<P: AsRef<Path>>(base_path: P, index: IndexStorage) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            index,
        }
    }

    pub fn default_path() -> Result<PathBuf> {
        let project_root = std::env::current_dir().context("Failed to get current directory")?;
        Ok(project_root.join("../../.hoosh.bak").join("conversations"))
    }

    pub fn with_default_path() -> Result<Self> {
        let path = Self::default_path()?;
        Self::new(path)
    }

    fn conversation_dir(&self, conversation_id: &str) -> PathBuf {
        self.base_path.join(conversation_id)
    }

    fn messages_file(&self, conversation_id: &str) -> PathBuf {
        self.conversation_dir(conversation_id)
            .join("messages.jsonl")
    }

    fn metadata_file(&self, conversation_id: &str) -> PathBuf {
        self.conversation_dir(conversation_id).join("meta.json")
    }

    pub fn generate_conversation_id() -> String {
        let now = chrono::Local::now();
        format!("conv_{}", now.format("%Y%m%d_%H%M%S"))
    }

    pub fn create_conversation(&self, conversation_id: &str) -> Result<ConversationMetadata> {
        let conv_dir = self.conversation_dir(conversation_id);
        fs::create_dir_all(&conv_dir).context("Failed to create conversation directory")?;

        let metadata = ConversationMetadata::new(conversation_id.to_string());
        self.save_metadata(&metadata)?;
        self.index.add_conversation(&metadata)?;

        Ok(metadata)
    }

    pub fn save_metadata(&self, metadata: &ConversationMetadata) -> Result<()> {
        let metadata_path = self.metadata_file(&metadata.id);
        let json =
            serde_json::to_string_pretty(metadata).context("Failed to serialize metadata")?;

        fs::write(&metadata_path, json).context("Failed to write metadata file")?;

        self.index.update_conversation(metadata)?;

        Ok(())
    }

    pub fn load_metadata(&self, conversation_id: &str) -> Result<ConversationMetadata> {
        let metadata_path = self.metadata_file(conversation_id);
        let content = fs::read_to_string(&metadata_path).context("Failed to read metadata file")?;

        let metadata: ConversationMetadata =
            serde_json::from_str(&content).context("Failed to parse metadata")?;

        Ok(metadata)
    }

    pub fn append_message(
        &self,
        conversation_id: &str,
        message: &ConversationMessage,
    ) -> Result<()> {
        let messages_path = self.messages_file(conversation_id);

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&messages_path)
            .context("Failed to open messages file")?;

        let json = serde_json::to_string(message).context("Failed to serialize message")?;

        writeln!(file, "{}", json).context("Failed to write message")?;

        let mut metadata = self.load_metadata(conversation_id)?;
        metadata.message_count += 1;
        metadata.update();
        self.save_metadata(&metadata)?;

        Ok(())
    }

    pub fn load_messages(&self, conversation_id: &str) -> Result<Vec<ConversationMessage>> {
        let messages_path = self.messages_file(conversation_id);

        if !messages_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&messages_path).context("Failed to read messages file")?;

        let mut messages = Vec::new();
        for (line_num, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }

            let message: ConversationMessage = serde_json::from_str(line)
                .with_context(|| format!("Failed to parse message at line {}", line_num + 1))?;

            messages.push(message);
        }

        Ok(messages)
    }

    pub fn update_title(&self, conversation_id: &str, title: String) -> Result<()> {
        let mut metadata = self.load_metadata(conversation_id)?;
        metadata.title = title;
        metadata.update();
        self.save_metadata(&metadata)?;
        Ok(())
    }

    pub fn conversation_exists(&self, conversation_id: &str) -> bool {
        self.conversation_dir(conversation_id).exists()
    }

    pub fn list_conversations(&self) -> Result<Vec<ConversationMetadata>> {
        self.index.list_conversations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_storage() -> (ConversationStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("index.json");
        let index = IndexStorage::new(&index_path);
        let storage = ConversationStorage::new_with_index(temp_dir.path(), index);
        (storage, temp_dir)
    }

    fn create_test_message(role: &str, content: &str) -> ConversationMessage {
        ConversationMessage {
            role: role.to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    #[test]
    fn test_create_conversation() {
        let (storage, _temp) = create_test_storage();
        let conv_id = "test_conv_001";

        let metadata = storage.create_conversation(conv_id).unwrap();

        assert_eq!(metadata.id, conv_id);
        assert_eq!(metadata.message_count, 0);
        assert!(storage.conversation_exists(conv_id));
    }

    #[test]
    fn test_append_and_load_messages() {
        let (storage, _temp) = create_test_storage();
        let conv_id = "test_conv_002";

        storage.create_conversation(conv_id).unwrap();

        let msg1 = create_test_message("user", "Hello");
        let msg2 = create_test_message("assistant", "Hi there!");

        storage.append_message(conv_id, &msg1).unwrap();
        storage.append_message(conv_id, &msg2).unwrap();

        let loaded = storage.load_messages(conv_id).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].role, "user");
        assert_eq!(loaded[0].content, Some("Hello".to_string()));
        assert_eq!(loaded[1].role, "assistant");
        assert_eq!(loaded[1].content, Some("Hi there!".to_string()));

        let metadata = storage.load_metadata(conv_id).unwrap();
        assert_eq!(metadata.message_count, 2);
    }

    #[test]
    fn test_update_title() {
        let (storage, _temp) = create_test_storage();
        let conv_id = "test_conv_003";

        storage.create_conversation(conv_id).unwrap();
        storage
            .update_title(conv_id, "Test Conversation".to_string())
            .unwrap();

        let metadata = storage.load_metadata(conv_id).unwrap();
        assert_eq!(metadata.title, "Test Conversation");
    }

    #[test]
    fn test_list_conversations() {
        let (storage, _temp) = create_test_storage();

        storage.create_conversation("conv_001").unwrap();
        storage.create_conversation("conv_002").unwrap();
        storage.create_conversation("conv_003").unwrap();

        storage
            .update_title("conv_001", "First".to_string())
            .unwrap();
        storage
            .update_title("conv_002", "Second".to_string())
            .unwrap();
        storage
            .update_title("conv_003", "Third".to_string())
            .unwrap();

        let conversations = storage.list_conversations().unwrap();
        assert_eq!(conversations.len(), 3);
    }

    #[test]
    fn test_generate_conversation_id() {
        let id = ConversationStorage::generate_conversation_id();
        assert!(id.starts_with("conv_"));
        assert!(id.len() > 5);
    }

    #[test]
    fn test_metadata_timestamps() {
        let (storage, _temp) = create_test_storage();
        let conv_id = "test_conv_004";

        let metadata = storage.create_conversation(conv_id).unwrap();
        let created_at = metadata.created_at;

        std::thread::sleep(std::time::Duration::from_secs(1));

        let msg = create_test_message("user", "Test");
        storage.append_message(conv_id, &msg).unwrap();

        let updated_metadata = storage.load_metadata(conv_id).unwrap();
        assert_eq!(updated_metadata.created_at, created_at);
        assert!(updated_metadata.updated_at >= created_at);
    }
}

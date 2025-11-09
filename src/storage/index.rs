use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::ConversationMetadata;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationIndex {
    conversations: HashMap<String, ConversationMetadata>,
}

impl ConversationIndex {
    pub fn new() -> Self {
        Self {
            conversations: HashMap::new(),
        }
    }

    pub fn add(&mut self, metadata: ConversationMetadata) {
        self.conversations.insert(metadata.id.clone(), metadata);
    }

    pub fn update(&mut self, metadata: ConversationMetadata) {
        self.conversations.insert(metadata.id.clone(), metadata);
    }

    pub fn remove(&mut self, conversation_id: &str) {
        self.conversations.remove(conversation_id);
    }

    pub fn get(&self, conversation_id: &str) -> Option<&ConversationMetadata> {
        self.conversations.get(conversation_id)
    }

    pub fn list(&self) -> Vec<ConversationMetadata> {
        let mut conversations: Vec<_> = self.conversations.values().cloned().collect();
        conversations.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        conversations
    }

    pub fn len(&self) -> usize {
        self.conversations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.conversations.is_empty()
    }
}

impl Default for ConversationIndex {
    fn default() -> Self {
        Self::new()
    }
}

pub struct IndexStorage {
    index_path: PathBuf,
}

impl IndexStorage {
    pub fn new<P: AsRef<Path>>(index_path: P) -> Self {
        Self {
            index_path: index_path.as_ref().to_path_buf(),
        }
    }

    pub fn default_path() -> Result<PathBuf> {
        let project_root = std::env::current_dir().context("Failed to get current directory")?;
        Ok(project_root
            .join(".hoosh")
            .join("conversations")
            .join("index.json"))
    }

    pub fn with_default_path() -> Result<Self> {
        let path = Self::default_path()?;
        Ok(Self::new(path))
    }

    pub fn load(&self) -> Result<ConversationIndex> {
        if !self.index_path.exists() {
            return Ok(ConversationIndex::new());
        }

        let content = fs::read_to_string(&self.index_path).context("Failed to read index file")?;

        let index: ConversationIndex =
            serde_json::from_str(&content).context("Failed to parse index file")?;

        Ok(index)
    }

    pub fn save(&self, index: &ConversationIndex) -> Result<()> {
        if let Some(parent) = self.index_path.parent() {
            fs::create_dir_all(parent).context("Failed to create index directory")?;
        }

        let json = serde_json::to_string_pretty(index).context("Failed to serialize index")?;

        fs::write(&self.index_path, json).context("Failed to write index file")?;

        Ok(())
    }

    pub fn add_conversation(&self, metadata: &ConversationMetadata) -> Result<()> {
        let mut index = self.load()?;
        index.add(metadata.clone());
        self.save(&index)?;
        Ok(())
    }

    pub fn update_conversation(&self, metadata: &ConversationMetadata) -> Result<()> {
        let mut index = self.load()?;
        index.update(metadata.clone());
        self.save(&index)?;
        Ok(())
    }

    pub fn remove_conversation(&self, conversation_id: &str) -> Result<()> {
        let mut index = self.load()?;
        index.remove(conversation_id);
        self.save(&index)?;
        Ok(())
    }

    pub fn list_conversations(&self) -> Result<Vec<ConversationMetadata>> {
        let index = self.load()?;
        Ok(index.list())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_metadata(id: &str, title: &str) -> ConversationMetadata {
        ConversationMetadata {
            id: id.to_string(),
            title: title.to_string(),
            created_at: 1234567890,
            updated_at: 1234567890,
            message_count: 0,
        }
    }

    #[test]
    fn test_index_add_and_list() {
        let mut index = ConversationIndex::new();
        assert!(index.is_empty());

        let meta1 = create_test_metadata("conv_001", "First");
        let meta2 = create_test_metadata("conv_002", "Second");

        index.add(meta1.clone());
        index.add(meta2.clone());

        assert_eq!(index.len(), 2);
        assert!(!index.is_empty());

        let list = index.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_index_get() {
        let mut index = ConversationIndex::new();
        let meta = create_test_metadata("conv_001", "Test");

        index.add(meta.clone());

        assert!(index.get("conv_001").is_some());
        assert_eq!(index.get("conv_001").unwrap().title, "Test");
        assert!(index.get("nonexistent").is_none());
    }

    #[test]
    fn test_index_update() {
        let mut index = ConversationIndex::new();
        let mut meta = create_test_metadata("conv_001", "Original");

        index.add(meta.clone());
        assert_eq!(index.get("conv_001").unwrap().title, "Original");

        meta.title = "Updated".to_string();
        index.update(meta);

        assert_eq!(index.get("conv_001").unwrap().title, "Updated");
    }

    #[test]
    fn test_index_remove() {
        let mut index = ConversationIndex::new();
        let meta = create_test_metadata("conv_001", "Test");

        index.add(meta);
        assert_eq!(index.len(), 1);

        index.remove("conv_001");
        assert_eq!(index.len(), 0);
        assert!(index.get("conv_001").is_none());
    }

    #[test]
    fn test_index_sorting() {
        let mut index = ConversationIndex::new();

        let mut meta1 = create_test_metadata("conv_001", "First");
        meta1.updated_at = 1000;

        let mut meta2 = create_test_metadata("conv_002", "Second");
        meta2.updated_at = 3000;

        let mut meta3 = create_test_metadata("conv_003", "Third");
        meta3.updated_at = 2000;

        index.add(meta1);
        index.add(meta2);
        index.add(meta3);

        let list = index.list();
        assert_eq!(list[0].id, "conv_002");
        assert_eq!(list[1].id, "conv_003");
        assert_eq!(list[2].id, "conv_001");
    }

    #[test]
    fn test_index_storage_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("index.json");
        let storage = IndexStorage::new(&index_path);

        let mut index = ConversationIndex::new();
        let meta1 = create_test_metadata("conv_001", "First");
        let meta2 = create_test_metadata("conv_002", "Second");

        index.add(meta1);
        index.add(meta2);

        storage.save(&index).unwrap();
        assert!(index_path.exists());

        let loaded = storage.load().unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded.get("conv_001").is_some());
        assert!(loaded.get("conv_002").is_some());
    }

    #[test]
    fn test_index_storage_add_conversation() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("index.json");
        let storage = IndexStorage::new(&index_path);

        let meta = create_test_metadata("conv_001", "Test");
        storage.add_conversation(&meta).unwrap();

        let list = storage.list_conversations().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "conv_001");
    }

    #[test]
    fn test_index_storage_update_conversation() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("index.json");
        let storage = IndexStorage::new(&index_path);

        let mut meta = create_test_metadata("conv_001", "Original");
        storage.add_conversation(&meta).unwrap();

        meta.title = "Updated".to_string();
        storage.update_conversation(&meta).unwrap();

        let list = storage.list_conversations().unwrap();
        assert_eq!(list[0].title, "Updated");
    }

    #[test]
    fn test_index_storage_remove_conversation() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("index.json");
        let storage = IndexStorage::new(&index_path);

        let meta = create_test_metadata("conv_001", "Test");
        storage.add_conversation(&meta).unwrap();
        assert_eq!(storage.list_conversations().unwrap().len(), 1);

        storage.remove_conversation("conv_001").unwrap();
        assert_eq!(storage.list_conversations().unwrap().len(), 0);
    }

    #[test]
    fn test_index_storage_load_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("nonexistent.json");
        let storage = IndexStorage::new(&index_path);

        let index = storage.load().unwrap();
        assert!(index.is_empty());
    }
}

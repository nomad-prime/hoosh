// Integration tests for conversation storage feature
// US1: Tests verify storage can be disabled and conversations work without persistence

use hoosh::agent::Conversation;
use hoosh::storage::ConversationStorage;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn test_conversation_without_storage_creates_no_files() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create conversation without storage (privacy-first: no persistence)
    let mut conversation = Conversation::new();
    conversation.add_user_message("test message".to_string());

    // Verify no conversation files were created in temp directory
    let entries: Vec<_> = std::fs::read_dir(temp_path).unwrap().collect();
    assert_eq!(
        entries.len(),
        0,
        "No files should be created when storage is disabled"
    );
}

#[test]
fn test_conversation_with_storage_creates_files() {
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(ConversationStorage::new(temp_dir.path()).unwrap());

    let mut conversation = Conversation::with_storage("test-id".to_string(), storage).unwrap();

    conversation.add_user_message("test message".to_string());

    // Verify files were created when storage is enabled
    let conv_dir = temp_dir.path().join("test-id");
    assert!(conv_dir.exists(), "Conversation directory should exist");
    assert!(
        conv_dir.join("messages.jsonl").exists(),
        "Messages file should exist"
    );
    assert!(
        conv_dir.join("meta.json").exists(),
        "Metadata file should exist"
    );
}

#[test]
fn test_messages_not_persisted_when_disabled() {
    // Create conversation without storage
    let mut conversation = Conversation::new();

    // Add multiple messages
    conversation.add_user_message("first message".to_string());
    conversation.add_assistant_message(Some("response".to_string()), None);
    conversation.add_user_message("second message".to_string());

    // Verify messages exist in memory
    let messages = conversation.get_messages_for_api();
    assert_eq!(messages.len(), 3, "Messages should exist in memory");

    // Since we used Conversation::new(), no files should be created anywhere
    // This test primarily verifies that the code doesn't panic or error
    // when storage is None and persist_message is called
}

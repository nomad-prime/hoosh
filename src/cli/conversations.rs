use crate::cli::ConversationsAction;
use crate::{AppConfig, ConversationStorage, console};
use std::path::PathBuf;

pub fn handle_conversations(action: ConversationsAction, config: &AppConfig) -> anyhow::Result<()> {
    match action {
        ConversationsAction::List => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let storage = match config.conversation_storage_root(&cwd)? {
                Some(root) => ConversationStorage::with_root(&root),
                None => {
                    console().plain(
                        "Conversation storage is disabled (conversation_storage = \"off\").",
                    );
                    return Ok(());
                }
            };
            let conversations = storage.list_conversations()?;

            if conversations.is_empty() {
                console().plain("No conversations found.");
                return Ok(());
            }

            for conv in conversations {
                let label = conv
                    .name
                    .as_deref()
                    .map(|n| format!("[{}]", n))
                    .unwrap_or_default();
                console().plain(&format!(
                    "{:<25} {:<20} {}",
                    conv.id, label, conv.title
                ));
            }
        }
    }
    Ok(())
}

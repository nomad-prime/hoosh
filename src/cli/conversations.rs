use crate::cli::ConversationsAction;
use crate::{ConversationStorage, console};

pub fn handle_conversations(action: ConversationsAction) -> anyhow::Result<()> {
    match action {
        ConversationsAction::List => {
            let storage = ConversationStorage::with_default_path()?;
            let conversations = storage.list_conversations()?;

            if conversations.is_empty() {
                console().plain("No conversations found.");
                return Ok(());
            }

            for conv in conversations {
                console().plain(&format!("{:<25} {}", conv.id, conv.title));
            }
        }
    }
    Ok(())
}

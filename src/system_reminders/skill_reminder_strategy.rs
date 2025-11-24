use crate::Conversation;
use crate::skill_management::SkillManager;
use crate::system_reminders::{ReminderContext, ReminderStrategy, SideEffectResult};
use anyhow::Result;
use std::path::PathBuf;

pub struct SkillReminderStrategy {
    skill_manager: SkillManager,
    project_root: PathBuf,
}

impl SkillReminderStrategy {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            skill_manager: SkillManager::new(),
            project_root,
        }
    }
}

#[async_trait::async_trait]
impl ReminderStrategy for SkillReminderStrategy {
    async fn apply(
        &self,
        conversation: &mut Conversation,
        _context: &ReminderContext,
    ) -> Result<SideEffectResult> {
        if let Ok(skills) = self.skill_manager.discover_skills(&self.project_root) {
            let summary = self.skill_manager.get_skills_summary(&skills);

            if !summary.is_empty()
                && let Some(last_msg) = conversation.messages.last()
                && last_msg.role == "user"
            {
                conversation.add_system_message(summary);
            }
        }

        Ok(SideEffectResult::Continue)
    }

    fn name(&self) -> &'static str {
        "skill_reminder"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_context() -> ReminderContext {
        ReminderContext { agent_step: 0 }
    }

    #[test]
    fn test_strategy_name() {
        let strategy = SkillReminderStrategy::new(PathBuf::from("."));
        assert_eq!(strategy.name(), "skill_reminder");
    }

    #[tokio::test]
    async fn test_injects_skills_into_user_message() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path().join(".hoosh").join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let skill_path = skills_dir.join("test_skill.sh");
        fs::write(&skill_path, "#!/bin/bash\n# Test skill\necho 'test'").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&skill_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&skill_path, perms).unwrap();
        }

        let strategy = SkillReminderStrategy::new(temp_dir.path().to_path_buf());
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        let last_msg = conversation.messages.last().unwrap();
        let content = last_msg.content.as_ref().unwrap();
        assert!(content.contains("available_skills"));
        assert!(content.contains("test_skill"));
    }

    #[tokio::test]
    async fn test_no_injection_without_skills() {
        let temp_dir = TempDir::new().unwrap();
        let strategy = SkillReminderStrategy::new(temp_dir.path().to_path_buf());
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        let last_msg = conversation.messages.last().unwrap();
        let content = last_msg.content.as_ref().unwrap();
        // If no skills, no summary should be added
        assert_eq!(content, "Hello");
    }

    #[tokio::test]
    async fn test_no_injection_without_user_message() {
        let temp_dir = TempDir::new().unwrap();
        let strategy = SkillReminderStrategy::new(temp_dir.path().to_path_buf());
        let mut conversation = Conversation::new();

        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        assert!(conversation.messages.is_empty());
    }

    #[tokio::test]
    async fn test_always_returns_continue() {
        let temp_dir = TempDir::new().unwrap();
        let strategy = SkillReminderStrategy::new(temp_dir.path().to_path_buf());
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), SideEffectResult::Continue));
    }

    #[tokio::test]
    async fn test_no_injection_when_summary_is_empty() {
        let temp_dir = TempDir::new().unwrap();
        let strategy = SkillReminderStrategy::new(temp_dir.path().to_path_buf());
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let initial_count = conversation.messages.len();
        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        // No skill summary should be injected when summary is empty
        assert_eq!(conversation.messages.len(), initial_count);
        let last_msg = conversation.messages.last().unwrap();
        assert_eq!(last_msg.content.as_ref().unwrap(), "Hello");
    }

    #[tokio::test]
    async fn test_no_injection_when_last_message_is_assistant() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path().join(".hoosh").join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let skill_path = skills_dir.join("test_skill.sh");
        fs::write(&skill_path, "#!/bin/bash\n# Test skill\necho 'test'").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&skill_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&skill_path, perms).unwrap();
        }

        let strategy = SkillReminderStrategy::new(temp_dir.path().to_path_buf());
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());
        // Add an assistant message as the last message
        conversation.add_assistant_message(Some("Response".to_string()), None);

        let initial_count = conversation.messages.len();
        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        // No injection should occur when last message is not from user
        assert_eq!(conversation.messages.len(), initial_count);
        let last_msg = conversation.messages.last().unwrap();
        assert_eq!(last_msg.role, "assistant");
    }

    #[tokio::test]
    async fn test_no_injection_when_last_message_is_system() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path().join(".hoosh").join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let skill_path = skills_dir.join("test_skill.sh");
        fs::write(&skill_path, "#!/bin/bash\n# Test skill\necho 'test'").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&skill_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&skill_path, perms).unwrap();
        }

        let strategy = SkillReminderStrategy::new(temp_dir.path().to_path_buf());
        let mut conversation = Conversation::new();
        // Add a system message as the last message
        conversation.add_system_message("System info".to_string());

        let initial_count = conversation.messages.len();
        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        // No injection should occur when last message is not from user
        assert_eq!(conversation.messages.len(), initial_count);
        let last_msg = conversation.messages.last().unwrap();
        assert_eq!(last_msg.role, "system");
    }

    #[tokio::test]
    async fn test_no_injection_with_empty_summary_but_user_last_message() {
        let temp_dir = TempDir::new().unwrap();
        let strategy = SkillReminderStrategy::new(temp_dir.path().to_path_buf());
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let initial_count = conversation.messages.len();
        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        // Even though last message is user, no injection if summary is empty
        assert_eq!(conversation.messages.len(), initial_count);
    }

    #[tokio::test]
    async fn test_injection_only_with_skills_and_user_last_message() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path().join(".hoosh").join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let skill_path = skills_dir.join("my_skill.sh");
        fs::write(&skill_path, "#!/bin/bash\n# My skill\necho 'skill'").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&skill_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&skill_path, perms).unwrap();
        }

        let strategy = SkillReminderStrategy::new(temp_dir.path().to_path_buf());
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let initial_count = conversation.messages.len();
        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        // Injection should occur: skills exist AND last message is user
        assert_eq!(conversation.messages.len(), initial_count + 1);
        let last_msg = conversation.messages.last().unwrap();
        assert_eq!(last_msg.role, "system");
        let content = last_msg.content.as_ref().unwrap();
        assert!(content.contains("available_skills"));
    }

    #[tokio::test]
    async fn test_handles_discovery_error_gracefully() {
        // Use a non-existent path that might cause discovery to fail
        let strategy =
            SkillReminderStrategy::new(PathBuf::from("/nonexistent/path/that/does/not/exist"));
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        // Should still return Ok(Continue) even if skill discovery fails
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), SideEffectResult::Continue));
        // Message should remain unchanged
        let last_msg = conversation.messages.last().unwrap();
        assert_eq!(last_msg.content.as_ref().unwrap(), "Hello");
    }
}

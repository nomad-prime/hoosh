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

            if !summary.is_empty() {
                if let Some(last_msg) = conversation.messages.last_mut()
                    && last_msg.role == "user"
                    && let Some(content) = &mut last_msg.content
                {
                    content.push_str("\n\n");
                    content.push_str(&summary);
                }
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
}

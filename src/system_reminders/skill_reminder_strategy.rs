use crate::Conversation;
use crate::agent::Role;
use crate::skill_management::SkillManager;
use crate::system_reminders::{ReminderContext, ReminderStrategy, SideEffectResult};
use anyhow::Result;
use std::path::PathBuf;

pub struct SkillReminderStrategy {
    skill_manager: SkillManager,
}

impl SkillReminderStrategy {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            skill_manager: SkillManager::with_roots(roots),
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
        if let Ok(skills) = self.skill_manager.discover_skills() {
            let summary = self.skill_manager.get_skills_summary(&skills);

            if !summary.is_empty()
                && let Some(last_msg) = conversation.messages.last()
                && last_msg.role == Role::User
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

    fn make_skills_dir(tmp: &TempDir) -> PathBuf {
        let dir = tmp.path().join(".hoosh").join("skills");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_executable(path: &std::path::Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms).unwrap();
        }
    }

    fn create_context() -> ReminderContext {
        ReminderContext { agent_step: 0 }
    }

    #[test]
    fn test_strategy_name() {
        let strategy = SkillReminderStrategy::new(vec![]);
        assert_eq!(strategy.name(), "skill_reminder");
    }

    #[tokio::test]
    async fn test_injects_skills_into_user_message() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = make_skills_dir(&tmp);
        let skill_path = skills_dir.join("test_skill.sh");
        fs::write(&skill_path, "#!/bin/bash\n# Test skill\necho 'test'").unwrap();
        make_executable(&skill_path);

        let strategy = SkillReminderStrategy::new(vec![skills_dir]);
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let result = strategy.apply(&mut conversation, &create_context()).await;

        assert!(result.is_ok());
        let last_msg = conversation.messages.last().unwrap();
        let content = last_msg.content.as_ref().unwrap();
        assert!(content.contains("available_skills"));
        assert!(content.contains("test_skill"));
    }

    #[tokio::test]
    async fn test_no_injection_without_skills() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = make_skills_dir(&tmp);
        let strategy = SkillReminderStrategy::new(vec![skills_dir]);
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let result = strategy.apply(&mut conversation, &create_context()).await;

        assert!(result.is_ok());
        let last_msg = conversation.messages.last().unwrap();
        assert_eq!(last_msg.content.as_ref().unwrap(), "Hello");
    }

    #[tokio::test]
    async fn test_no_injection_without_user_message() {
        let strategy = SkillReminderStrategy::new(vec![]);
        let mut conversation = Conversation::new();

        let result = strategy.apply(&mut conversation, &create_context()).await;

        assert!(result.is_ok());
        assert!(conversation.messages.is_empty());
    }

    #[tokio::test]
    async fn test_always_returns_continue() {
        let strategy = SkillReminderStrategy::new(vec![]);
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let result = strategy.apply(&mut conversation, &create_context()).await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), SideEffectResult::Continue));
    }

    #[tokio::test]
    async fn test_no_injection_when_last_message_is_assistant() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = make_skills_dir(&tmp);
        let skill_path = skills_dir.join("test_skill.sh");
        fs::write(&skill_path, "#!/bin/bash\n# Test skill\necho 'test'").unwrap();
        make_executable(&skill_path);

        let strategy = SkillReminderStrategy::new(vec![skills_dir]);
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());
        conversation.add_assistant_message(Some("Response".to_string()), None);

        let initial_count = conversation.messages.len();
        let result = strategy.apply(&mut conversation, &create_context()).await;

        assert!(result.is_ok());
        assert_eq!(conversation.messages.len(), initial_count);
    }

    #[tokio::test]
    async fn test_no_injection_when_last_message_is_system() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = make_skills_dir(&tmp);
        let skill_path = skills_dir.join("test_skill.sh");
        fs::write(&skill_path, "#!/bin/bash\n# Test skill\necho 'test'").unwrap();
        make_executable(&skill_path);

        let strategy = SkillReminderStrategy::new(vec![skills_dir]);
        let mut conversation = Conversation::new();
        conversation.add_system_message("System info".to_string());

        let initial_count = conversation.messages.len();
        let result = strategy.apply(&mut conversation, &create_context()).await;

        assert!(result.is_ok());
        assert_eq!(conversation.messages.len(), initial_count);
    }

    #[tokio::test]
    async fn test_injection_with_skill_md_format() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = make_skills_dir(&tmp);
        let skill_dir = skills_dir.join("pdf-processing");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: pdf-processing\ndescription: Extract text from PDFs.\n---\n\nInstructions here.",
        )
        .unwrap();

        let strategy = SkillReminderStrategy::new(vec![skills_dir]);
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let initial_count = conversation.messages.len();
        let result = strategy.apply(&mut conversation, &create_context()).await;

        assert!(result.is_ok());
        assert_eq!(conversation.messages.len(), initial_count + 1);
        let content = conversation
            .messages
            .last()
            .unwrap()
            .content
            .as_ref()
            .unwrap();
        assert!(content.contains("pdf-processing"));
        assert!(content.contains("Extract text from PDFs."));
        assert!(content.contains("SKILL.md"));
    }

    #[tokio::test]
    async fn test_handles_missing_roots_gracefully() {
        let strategy = SkillReminderStrategy::new(vec![PathBuf::from(
            "/nonexistent/path/that/does/not/exist",
        )]);
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let result = strategy.apply(&mut conversation, &create_context()).await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), SideEffectResult::Continue));
        let last_msg = conversation.messages.last().unwrap();
        assert_eq!(last_msg.content.as_ref().unwrap(), "Hello");
    }
}

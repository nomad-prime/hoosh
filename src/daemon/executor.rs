use anyhow::{Context, Result};
use chrono::Utc;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::mpsc;

use crate::agent::{Agent, AgentEvent, Conversation};
use crate::backends::LlmBackend;
use crate::daemon::config::DaemonConfig;
use crate::daemon::permissions::PermissionResolver;
use crate::daemon::sandbox::Sandbox;
use crate::daemon::store::TaskStore;
use crate::daemon::task::{Task, TaskStatus};
use crate::permissions::PermissionManager;
use crate::permissions::storage::PermissionsFile;
use crate::system_reminders::{PeriodicCoreReminderStrategy, SystemReminder};
use crate::tool_executor::ToolExecutor;
use crate::tools::{BuiltinToolProvider, ToolRegistry};

const HOOSH_DAEMON_CODER_PROMPT: &str = include_str!("../prompts/hoosh_daemon_coder.txt");
const HOOSH_DAEMON_CODER_CORE_INSTRUCTIONS: &str =
    include_str!("../prompts/hoosh_daemon_coder_core_instructions.txt");

pub struct TaskExecutor {
    pub store: Arc<TaskStore>,
    pub config: Arc<DaemonConfig>,
    pub backend: Arc<dyn LlmBackend>,
}

impl TaskExecutor {
    pub fn new(
        store: Arc<TaskStore>,
        config: Arc<DaemonConfig>,
        backend: Arc<dyn LlmBackend>,
    ) -> Self {
        Self {
            store,
            config,
            backend,
        }
    }

    pub async fn run(self: Arc<Self>, task_id: String, cancel: Arc<AtomicBool>) {
        if let Err(e) = self.execute(&task_id, Arc::clone(&cancel)).await
            && let Ok(Some(mut task)) = self.store.get(&task_id)
            && !task.status.is_terminal()
        {
            task.status = TaskStatus::Failed;
            task.error_message = Some(e.to_string());
            task.completed_at = Some(Utc::now());
            let _ = self.store.update(&task);
        }
    }

    async fn execute(&self, task_id: &str, cancel: Arc<AtomicBool>) -> Result<()> {
        let mut task = self
            .store
            .get(task_id)?
            .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_id))?;

        task.status = TaskStatus::Running;
        task.started_at = Some(Utc::now());
        self.store.update(&task)?;

        let mut sandbox = Sandbox::create(&task.id, &self.config.sandbox_base_dir)
            .await
            .context("Failed to create sandbox")?;

        task.log_path = Some(sandbox.log_path());
        task.sandbox_path = Some(sandbox.sandbox_dir.clone());
        self.store.update(&task)?;

        let _ = writeln!(sandbox, "[{}] Clone started: {}", Utc::now(), task.repo_url);

        let clone_result = sandbox
            .clone(
                &task.repo_url,
                &task.base_branch,
                self.config.ssh_key_path.as_ref(),
            )
            .await;

        if let Err(e) = clone_result {
            let _ = writeln!(sandbox, "[{}] Clone failed: {}", Utc::now(), e);
            task.status = TaskStatus::Failed;
            task.error_message = Some(format!("Clone failed: {}", e));
            task.completed_at = Some(Utc::now());
            self.store.update(&task)?;
            if !self.config.retain_sandboxes {
                let _ = sandbox.cleanup();
            }
            return Ok(());
        }

        let _ = writeln!(sandbox, "[{}] Clone completed", Utc::now());

        if task.trigger.is_some() {
            let gh_ok = tokio::process::Command::new("gh")
                .arg("auth")
                .arg("status")
                .output()
                .await
                .map(|o| o.status.success())
                .unwrap_or(false);
            if !gh_ok {
                let _ = writeln!(sandbox, "[{}] gh CLI not authenticated", Utc::now());
                task.status = TaskStatus::Failed;
                task.error_message = Some(
                    "gh CLI not authenticated — run 'gh auth login' on the daemon machine"
                        .to_string(),
                );
                task.completed_at = Some(Utc::now());
                self.store.update(&task)?;
                if !self.config.retain_sandboxes {
                    let _ = sandbox.cleanup();
                }
                return Ok(());
            }
        }

        let global_perms = PermissionResolver::load_global().unwrap_or_default();
        let repo_perms = PermissionResolver::load_repo(&sandbox.repo_dir);
        let merged_perms = PermissionResolver::resolve(global_perms, repo_perms);

        let _ = writeln!(sandbox, "[{}] Agent started", Utc::now());

        let repo_dir = sandbox.repo_dir.clone();
        let tokens = self
            .run_agent_turn(
                &task,
                &repo_dir,
                merged_perms,
                Arc::clone(&cancel),
                &mut sandbox,
            )
            .await?;

        task.tokens_consumed = tokens;

        if cancel.load(Ordering::Relaxed) {
            let incomplete_msg = if task.tokens_consumed >= task.token_budget {
                "[incomplete] token budget exceeded".to_string()
            } else {
                "[incomplete] task cancelled".to_string()
            };
            task.status = TaskStatus::Failed;
            task.error_message = Some(incomplete_msg);
        } else {
            task.status = TaskStatus::Completed;
        }
        task.completed_at = Some(Utc::now());
        self.store.update(&task)?;

        if !self.config.retain_sandboxes {
            let _ = sandbox.cleanup();
        }

        Ok(())
    }

    async fn run_agent_turn(
        &self,
        task: &Task,
        repo_dir: &Path,
        merged_perms: PermissionsFile,
        cancel: Arc<AtomicBool>,
        sandbox: &mut Sandbox,
    ) -> Result<usize> {
        let (event_tx, event_rx) = mpsc::unbounded_channel::<AgentEvent>();

        let budget = task.token_budget;
        let cancel_monitor = Arc::clone(&cancel);
        let token_count = Arc::new(AtomicUsize::new(0));
        let token_count_monitor = Arc::clone(&token_count);
        let log_path = sandbox.log_path();

        let monitor = tokio::spawn(async move {
            let mut log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .ok();

            let mut rx = event_rx;
            let mut total = 0usize;
            while let Some(event) = rx.recv().await {
                match &event {
                    AgentEvent::TokenUsage {
                        input_tokens,
                        output_tokens,
                        ..
                    } => {
                        total += input_tokens + output_tokens;
                        token_count_monitor.store(total, Ordering::Relaxed);
                        if total >= budget {
                            cancel_monitor.store(true, Ordering::Relaxed);
                        }
                    }
                    AgentEvent::ToolCalls(calls) => {
                        if let Some(ref mut f) = log {
                            for (name, input) in calls {
                                let _ =
                                    writeln!(f, "[{}] tool_call: {} {}", Utc::now(), name, input);
                            }
                        }
                    }
                    AgentEvent::ToolResult {
                        tool_name, summary, ..
                    } => {
                        if let Some(ref mut f) = log {
                            let _ = writeln!(
                                f,
                                "[{}] tool_result: {} -> {}",
                                Utc::now(),
                                tool_name,
                                summary
                            );
                        }
                    }
                    AgentEvent::PermissionDenied(tools) => {
                        if let Some(ref mut f) = log {
                            let _ = writeln!(
                                f,
                                "[{}] permission_denied: {}",
                                Utc::now(),
                                tools.join(", ")
                            );
                        }
                    }
                    _ => {}
                }
            }
        });

        {
            let perm_manager = Arc::new(
                PermissionManager::non_interactive(merged_perms)
                    .with_sandbox_root(repo_dir.to_path_buf()),
            );
            let tool_registry = Arc::new(
                ToolRegistry::new()
                    .with_provider(Arc::new(BuiltinToolProvider::new(repo_dir.to_path_buf()))),
            );
            let tool_executor = Arc::new(
                ToolExecutor::new(Arc::clone(&tool_registry), perm_manager)
                    .with_event_sender(event_tx.clone()),
            );

            let system_reminder = Arc::new(
                SystemReminder::new().add_strategy(Box::new(PeriodicCoreReminderStrategy::new(
                    10_000,
                    HOOSH_DAEMON_CODER_CORE_INSTRUCTIONS.to_string(),
                ))),
            );

            let agent = Agent::new(Arc::clone(&self.backend), tool_registry, tool_executor)
                .with_event_sender(event_tx.clone())
                .with_cancellation_token(Arc::clone(&cancel))
                .with_system_reminder(system_reminder);

            let mut conversation = Conversation::new();
            conversation.add_system_message(HOOSH_DAEMON_CODER_PROMPT.to_string());
            conversation.add_user_message(task.instructions.clone());

            let _ = agent.handle_turn(&mut conversation).await;
        }

        drop(event_tx);
        let _ = monitor.await;

        let total = token_count.load(Ordering::Relaxed);
        let _ = writeln!(
            sandbox,
            "[{}] Agent completed, tokens used: {}",
            Utc::now(),
            total
        );

        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::{LlmBackend, LlmResponse};
    use crate::daemon::config::DaemonConfig;
    use crate::daemon::store::TaskStore;
    use crate::daemon::task::Task;
    use anyhow::Result;
    use async_trait::async_trait;
    use std::sync::atomic::AtomicBool;
    use tempfile::TempDir;

    struct MockBackend {
        response: String,
        input_tokens: usize,
        output_tokens: usize,
    }

    impl MockBackend {
        fn simple(response: &str) -> Self {
            Self {
                response: response.to_string(),
                input_tokens: 10,
                output_tokens: 10,
            }
        }

        fn with_token_count(response: &str, input: usize, output: usize) -> Self {
            Self {
                response: response.to_string(),
                input_tokens: input,
                output_tokens: output,
            }
        }
    }

    #[async_trait]
    impl LlmBackend for MockBackend {
        async fn send_message(&self, _message: &str) -> Result<String> {
            Ok(self.response.clone())
        }

        async fn send_message_with_tools(
            &self,
            _conversation: &crate::agent::Conversation,
            _tools: &crate::tools::ToolRegistry,
        ) -> Result<LlmResponse, crate::backends::LlmError> {
            Ok(LlmResponse::content_only(self.response.clone())
                .with_tokens(self.input_tokens, self.output_tokens))
        }

        fn backend_name(&self) -> &str {
            "mock"
        }

        fn model_name(&self) -> &str {
            "mock-model"
        }
    }

    fn init_bare_with_commit(path: &std::path::Path) {
        let repo = git2::Repository::init_bare(path).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let tree_builder = repo.treebuilder(None).unwrap();
        let tree_oid = tree_builder.write().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        repo.commit(
            Some("refs/heads/main"),
            &sig,
            &sig,
            "Initial commit",
            &tree,
            &[],
        )
        .unwrap();
    }

    fn make_executor(
        store: Arc<TaskStore>,
        sandbox_dir: &TempDir,
        backend: Arc<dyn LlmBackend>,
    ) -> Arc<TaskExecutor> {
        let config = DaemonConfig {
            sandbox_base_dir: sandbox_dir.path().to_path_buf(),
            ..Default::default()
        };
        Arc::new(TaskExecutor::new(store, Arc::new(config), backend))
    }

    fn make_github_trigger(trigger_ref: &str, repo_url: &str) -> crate::daemon::task::GithubTrigger {
        use crate::daemon::task::{GithubEventType, GithubTrigger};
        GithubTrigger {
            event_type: GithubEventType::IssueComment,
            delivery_id: "test-delivery-1".to_string(),
            trigger_ref: trigger_ref.to_string(),
            repo_full_name: "owner/repo".to_string(),
            repo_url: repo_url.to_string(),
            default_branch: "main".to_string(),
            actor_login: "alice".to_string(),
            issue_or_pr_number: 47,
            comment_url: None,
            raw_payload: serde_json::json!({"action": "created"}),
        }
    }

    #[tokio::test]
    async fn no_changes_completes_without_pr() {
        let remote_dir = TempDir::new().unwrap();
        init_bare_with_commit(remote_dir.path());
        let repo_url = format!("file://{}", remote_dir.path().display());

        let store_dir = TempDir::new().unwrap();
        let store = Arc::new(TaskStore::new_with_dir(store_dir.path().join("tasks")).unwrap());

        let sandbox_dir = TempDir::new().unwrap();
        let backend = Arc::new(MockBackend::simple("Task complete, no changes needed."));
        let executor = make_executor(Arc::clone(&store), &sandbox_dir, backend);

        let task = Task::new(
            repo_url,
            "main".to_string(),
            "Do nothing".to_string(),
            None,
            100_000,
        );
        let task_id = task.id.clone();
        store.create(&task).unwrap();

        let cancel = Arc::new(AtomicBool::new(false));
        executor.run(task_id.clone(), cancel).await;

        let final_task = store.get(&task_id).unwrap().unwrap();
        assert_eq!(final_task.status, TaskStatus::Completed);
        assert!(final_task.pr_url.is_none());
    }

    #[tokio::test]
    async fn external_cancel_marks_failed_with_incomplete() {
        let remote_dir = TempDir::new().unwrap();
        init_bare_with_commit(remote_dir.path());
        let repo_url = format!("file://{}", remote_dir.path().display());

        let store_dir = TempDir::new().unwrap();
        let store = Arc::new(TaskStore::new_with_dir(store_dir.path().join("tasks")).unwrap());

        let sandbox_dir = TempDir::new().unwrap();
        let backend = Arc::new(MockBackend::simple("Task complete."));
        let executor = make_executor(Arc::clone(&store), &sandbox_dir, backend);

        let task = Task::new(
            repo_url,
            "main".to_string(),
            "Do something".to_string(),
            None,
            100_000,
        );
        let task_id = task.id.clone();
        store.create(&task).unwrap();

        let cancel = Arc::new(AtomicBool::new(true));
        executor.run(task_id.clone(), cancel).await;

        let final_task = store.get(&task_id).unwrap().unwrap();
        assert_eq!(final_task.status, TaskStatus::Failed);
        assert!(
            final_task
                .error_message
                .as_deref()
                .unwrap_or("")
                .contains("[incomplete]"),
            "Error message should contain [incomplete]"
        );
    }

    #[tokio::test]
    async fn webhook_task_runs_to_terminal_state() {
        let remote_dir = TempDir::new().unwrap();
        init_bare_with_commit(remote_dir.path());
        let repo_url = format!("file://{}", remote_dir.path().display());

        let store_dir = TempDir::new().unwrap();
        let store = Arc::new(TaskStore::new_with_dir(store_dir.path().join("tasks")).unwrap());

        let sandbox_dir = TempDir::new().unwrap();
        let backend = Arc::new(MockBackend::simple("Done."));
        let executor = make_executor(Arc::clone(&store), &sandbox_dir, backend);

        let mut task = Task::new(
            repo_url.clone(),
            "main".to_string(),
            "@hoosh fix the bug".to_string(),
            None,
            100_000,
        );
        task.trigger = Some(make_github_trigger("issue:47", &repo_url));
        let task_id = task.id.clone();
        store.create(&task).unwrap();

        let cancel = Arc::new(AtomicBool::new(false));
        executor.run(task_id.clone(), cancel).await;

        let final_task = store.get(&task_id).unwrap().unwrap();
        assert!(final_task.status.is_terminal());
        assert!(final_task.trigger.is_some());
    }

    #[tokio::test]
    async fn webhook_task_clone_failure_marks_failed() {
        let store_dir = TempDir::new().unwrap();
        let store = Arc::new(TaskStore::new_with_dir(store_dir.path().join("tasks")).unwrap());

        let sandbox_dir = TempDir::new().unwrap();
        let backend = Arc::new(MockBackend::simple("Done."));
        let executor = make_executor(Arc::clone(&store), &sandbox_dir, backend);

        let bad_url = "file:///nonexistent/path/repo";
        let mut task = Task::new(
            bad_url.to_string(),
            "main".to_string(),
            "@hoosh fix this".to_string(),
            None,
            100_000,
        );
        task.trigger = Some(make_github_trigger("issue:99", bad_url));
        let task_id = task.id.clone();
        store.create(&task).unwrap();

        let cancel = Arc::new(AtomicBool::new(false));
        executor.run(task_id.clone(), cancel).await;

        let final_task = store.get(&task_id).unwrap().unwrap();
        assert_eq!(final_task.status, TaskStatus::Failed);
        assert!(
            final_task.error_message.as_deref().unwrap_or("").contains("Clone failed"),
            "Error message should indicate clone failure"
        );
    }

    #[tokio::test]
    async fn token_exhaustion_sets_cancel_flag_and_marks_failed() {
        let remote_dir = TempDir::new().unwrap();
        init_bare_with_commit(remote_dir.path());
        let repo_url = format!("file://{}", remote_dir.path().display());

        let store_dir = TempDir::new().unwrap();
        let store = Arc::new(TaskStore::new_with_dir(store_dir.path().join("tasks")).unwrap());

        let sandbox_dir = TempDir::new().unwrap();
        // Backend reports 200 tokens per call, budget is 100 — should trigger cancellation
        let backend = Arc::new(MockBackend::with_token_count("Done.", 150, 150));
        let executor = make_executor(Arc::clone(&store), &sandbox_dir, backend);

        let task = Task::new(
            repo_url,
            "main".to_string(),
            "Do something".to_string(),
            Some(100),
            100,
        );
        let task_id = task.id.clone();
        store.create(&task).unwrap();

        let cancel = Arc::new(AtomicBool::new(false));
        executor.run(task_id.clone(), cancel.clone()).await;

        let final_task = store.get(&task_id).unwrap().unwrap();
        assert!(final_task.status.is_terminal());
        assert!(cancel.load(Ordering::Relaxed), "Cancel flag should be set");
    }
}

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
use crate::daemon::pr_provider::{CreatePrParams, PrProvider};
use crate::daemon::sandbox::Sandbox;
use crate::daemon::store::TaskStore;
use crate::daemon::task::{Task, TaskStatus};
use crate::permissions::PermissionManager;
use crate::permissions::storage::PermissionsFile;
use crate::tool_executor::ToolExecutor;
use crate::tools::{BuiltinToolProvider, ToolRegistry};

pub struct TaskExecutor {
    pub store: Arc<TaskStore>,
    pub config: Arc<DaemonConfig>,
    pub pr_provider: Arc<dyn PrProvider>,
    pub backend: Arc<dyn LlmBackend>,
}

impl TaskExecutor {
    pub fn new(
        store: Arc<TaskStore>,
        config: Arc<DaemonConfig>,
        pr_provider: Arc<dyn PrProvider>,
        backend: Arc<dyn LlmBackend>,
    ) -> Self {
        Self {
            store,
            config,
            pr_provider,
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

        let branch_name = format!("hoosh/{}", task.id);
        sandbox
            .create_branch(&branch_name)
            .context("Failed to create task branch")?;
        task.branch = Some(branch_name.clone());
        self.store.update(&task)?;

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
            let _ = writeln!(
                sandbox,
                "[{}] Task cancelled or token budget exceeded",
                Utc::now()
            );

            let incomplete_msg = if task.tokens_consumed >= task.token_budget {
                "[incomplete] token budget exceeded".to_string()
            } else {
                "[incomplete] task cancelled".to_string()
            };

            if sandbox.has_changes().unwrap_or(false) {
                let _ = sandbox.commit_all(&incomplete_msg);
                let _ = sandbox
                    .push(&branch_name, self.config.ssh_key_path.as_ref())
                    .await;
            }

            task.status = TaskStatus::Failed;
            task.error_message = Some(incomplete_msg);
            task.completed_at = Some(Utc::now());
            self.store.update(&task)?;
        } else if sandbox.has_changes()? {
            let _ = writeln!(sandbox, "[{}] Committing changes", Utc::now());

            let instructions_short = &task.instructions[..task.instructions.len().min(72)];
            let commit_msg = format!("hoosh: {}", instructions_short);
            sandbox.commit_all(&commit_msg)?;

            let _ = writeln!(sandbox, "[{}] Pushing branch: {}", Utc::now(), branch_name);
            sandbox
                .push(&branch_name, self.config.ssh_key_path.as_ref())
                .await?;

            let pr_title = task
                .pr_title
                .clone()
                .unwrap_or_else(|| format!("hoosh: {}", instructions_short));

            let _ = writeln!(sandbox, "[{}] Creating pull request", Utc::now());
            let pr_result = self
                .pr_provider
                .create_pull_request(CreatePrParams {
                    repo_url: task.repo_url.clone(),
                    head_branch: branch_name,
                    base_branch: task.base_branch.clone(),
                    title: pr_title,
                    body: format!(
                        "Automated PR created by hoosh daemon.\n\nInstructions: {}",
                        task.instructions
                    ),
                    labels: task.pr_labels.clone(),
                })
                .await?;

            let _ = writeln!(sandbox, "[{}] PR created: {}", Utc::now(), pr_result.pr_url);
            task.pr_url = Some(pr_result.pr_url);
            task.status = TaskStatus::Completed;
            task.completed_at = Some(Utc::now());
            self.store.update(&task)?;
        } else {
            let _ = writeln!(sandbox, "[{}] No changes, task completed", Utc::now());
            task.status = TaskStatus::Completed;
            task.completed_at = Some(Utc::now());
            self.store.update(&task)?;
        }

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

            let agent = Agent::new(Arc::clone(&self.backend), tool_registry, tool_executor)
                .with_event_sender(event_tx.clone())
                .with_cancellation_token(Arc::clone(&cancel));

            let mut conversation = Conversation::new();
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
    use crate::daemon::pr_provider::{CreatePrParams, PrProvider, PrResult};
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

    struct MockPrProvider {
        pr_url: String,
    }

    #[async_trait]
    impl PrProvider for MockPrProvider {
        async fn create_pull_request(&self, _params: CreatePrParams) -> Result<PrResult> {
            Ok(PrResult {
                pr_url: self.pr_url.clone(),
                pr_number: 1,
            })
        }

        fn provider_name(&self) -> &'static str {
            "mock"
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
        let mut config = DaemonConfig::default();
        config.sandbox_base_dir = sandbox_dir.path().to_path_buf();

        Arc::new(TaskExecutor::new(
            store,
            Arc::new(config),
            Arc::new(MockPrProvider {
                pr_url: "https://github.com/owner/repo/pull/1".to_string(),
            }),
            backend,
        ))
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

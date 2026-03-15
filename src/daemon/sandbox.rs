use anyhow::{Context, Result, bail};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tokio::time::Duration;

fn make_credential_callback(
    ssh_key: Option<PathBuf>,
) -> impl FnMut(&str, Option<&str>, git2::CredentialType) -> Result<git2::Cred, git2::Error> {
    move |_url, username_from_url, allowed| {
        let username = username_from_url.unwrap_or("git");
        if allowed.intersects(git2::CredentialType::SSH_KEY) {
            if let Some(ref key_path) = ssh_key {
                return git2::Cred::ssh_key(username, None, key_path, None);
            }
            if let Ok(cred) = git2::Cred::ssh_key_from_agent(username) {
                return Ok(cred);
            }
        }
        if allowed.intersects(git2::CredentialType::DEFAULT) {
            return git2::Cred::default();
        }
        Err(git2::Error::from_str("No supported credential type"))
    }
}

pub struct Sandbox {
    pub task_id: String,
    pub sandbox_dir: PathBuf,
    pub repo_dir: PathBuf,
    pub log_file: File,
    pub cloned: bool,
}

impl Sandbox {
    pub async fn create(task_id: &str, base_dir: &std::path::Path) -> Result<Sandbox> {
        let sandbox_dir = base_dir.join(format!("hoosh-{}", task_id));
        let repo_dir = sandbox_dir.join("repo");
        std::fs::create_dir_all(&repo_dir).with_context(|| {
            format!("Failed to create sandbox directory: {}", repo_dir.display())
        })?;

        let log_path = sandbox_dir.join("execution.log");
        let log_file = File::options()
            .create(true)
            .append(true)
            .open(&log_path)
            .with_context(|| format!("Failed to open log file: {}", log_path.display()))?;

        Ok(Sandbox {
            task_id: task_id.to_string(),
            sandbox_dir,
            repo_dir,
            log_file,
            cloned: false,
        })
    }

    pub async fn clone(
        &mut self,
        repo_url: &str,
        base_branch: &str,
        ssh_key_path: Option<&PathBuf>,
    ) -> Result<()> {
        let url = repo_url.to_string();
        let branch = base_branch.to_string();
        let repo_dir = self.repo_dir.clone();
        let ssh_key = ssh_key_path.cloned();

        let result = tokio::time::timeout(
            Duration::from_secs(300),
            tokio::task::spawn_blocking(move || -> Result<git2::Repository> {
                let mut fetch_opts = git2::FetchOptions::new();
                let mut callbacks = git2::RemoteCallbacks::new();
                callbacks.credentials(make_credential_callback(ssh_key));

                fetch_opts.remote_callbacks(callbacks);

                let mut builder = git2::build::RepoBuilder::new();
                builder.fetch_options(fetch_opts);
                builder.branch(&branch);

                builder
                    .clone(&url, &repo_dir)
                    .context("Failed to clone repository")
            }),
        )
        .await;

        match result {
            Ok(Ok(Ok(_repo))) => {
                self.cloned = true;
                Ok(())
            }
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(e)) => Err(anyhow::anyhow!("Clone task panicked: {}", e)),
            Err(_) => bail!("Repository clone timed out after 300 seconds"),
        }
    }

    fn open_repo(&self) -> Result<git2::Repository> {
        git2::Repository::open(&self.repo_dir).context("Failed to open repository")
    }

    pub fn create_branch(&self, branch_name: &str) -> Result<()> {
        let repo = self.open_repo()?;

        let head = repo.head().context("Failed to get HEAD")?;
        let commit = head
            .peel_to_commit()
            .context("Failed to peel HEAD to commit")?;

        repo.branch(branch_name, &commit, false)
            .with_context(|| format!("Failed to create branch '{}'", branch_name))?;

        let refname = format!("refs/heads/{}", branch_name);
        repo.set_head(&refname)
            .with_context(|| format!("Failed to set HEAD to '{}'", refname))?;

        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .context("Failed to checkout new branch")?;

        Ok(())
    }

    pub fn has_changes(&self) -> Result<bool> {
        let repo = self.open_repo()?;

        let statuses = repo
            .statuses(None)
            .context("Failed to get repository status")?;

        let changed = statuses.iter().any(|entry| {
            let status = entry.status();
            status.intersects(
                git2::Status::INDEX_NEW
                    | git2::Status::INDEX_MODIFIED
                    | git2::Status::INDEX_DELETED
                    | git2::Status::WT_NEW
                    | git2::Status::WT_MODIFIED
                    | git2::Status::WT_DELETED,
            )
        });

        Ok(changed)
    }

    pub fn commit_all(&self, message: &str) -> Result<()> {
        let repo = self.open_repo()?;

        let mut index = repo.index().context("Failed to get repository index")?;
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .context("Failed to stage changes")?;
        index.write().context("Failed to write index")?;

        let tree_oid = index.write_tree().context("Failed to write tree")?;
        let tree = repo.find_tree(tree_oid).context("Failed to find tree")?;

        let sig = repo.signature().unwrap_or_else(|_| {
            git2::Signature::now("hoosh", "hoosh@localhost").unwrap()
        });

        let parent_commit = repo
            .head()
            .context("Failed to get HEAD")?
            .peel_to_commit()
            .context("Failed to peel HEAD to commit")?;

        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent_commit])
            .context("Failed to create commit")?;

        Ok(())
    }

    pub async fn push(&self, branch_name: &str, ssh_key_path: Option<&PathBuf>) -> Result<()> {
        let repo_dir = self.repo_dir.clone();
        let branch = branch_name.to_string();
        let ssh_key = ssh_key_path.cloned();

        let result = tokio::time::timeout(
            Duration::from_secs(120),
            tokio::task::spawn_blocking(move || -> Result<()> {
                let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);
                let repo = git2::Repository::open(&repo_dir)
                    .context("Failed to open repository for push")?;

                let mut callbacks = git2::RemoteCallbacks::new();
                callbacks.credentials(make_credential_callback(ssh_key));

                let mut push_opts = git2::PushOptions::new();
                push_opts.remote_callbacks(callbacks);

                repo.find_remote("origin")
                    .context("Failed to find 'origin' remote")?
                    .push(&[&refspec], Some(&mut push_opts))
                    .context("Failed to push branch")?;

                Ok(())
            }),
        )
        .await;

        match result {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(e)) => Err(anyhow::anyhow!("Push task panicked: {}", e)),
            Err(_) => bail!("Repository push timed out after 120 seconds"),
        }
    }

    pub fn cleanup(&self) -> Result<()> {
        if self.sandbox_dir.exists() {
            std::fs::remove_dir_all(&self.sandbox_dir).with_context(|| {
                format!(
                    "Failed to remove sandbox directory: {}",
                    self.sandbox_dir.display()
                )
            })?;
        }
        Ok(())
    }

    pub fn log_path(&self) -> PathBuf {
        self.sandbox_dir.join("execution.log")
    }
}

impl Write for Sandbox {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.log_file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.log_file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn init_bare_with_commit(path: &std::path::Path) {
        let repo = git2::Repository::init_bare(path).unwrap();

        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let tree_oid = {
            let tree_builder = repo.treebuilder(None).unwrap();
            tree_builder.write().unwrap()
        };
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

    #[tokio::test]
    async fn create_makes_sandbox_directory() {
        let base = TempDir::new().unwrap();
        let sandbox = Sandbox::create("test-task-1", base.path())
            .await
            .unwrap();

        assert!(sandbox.repo_dir.exists());
        assert!(sandbox.sandbox_dir.exists());
    }

    #[tokio::test]
    async fn clone_creates_repo_at_sandbox_path() {
        let remote_dir = TempDir::new().unwrap();
        init_bare_with_commit(remote_dir.path());
        let url = format!("file://{}", remote_dir.path().display());

        let base = TempDir::new().unwrap();
        let mut sandbox = Sandbox::create("test-task-2", base.path())
            .await
            .unwrap();

        sandbox.clone(&url, "main", None).await.unwrap();

        assert!(sandbox.repo_dir.join(".git").exists());
    }

    #[tokio::test]
    async fn create_branch_checks_out_new_branch() {
        let remote_dir = TempDir::new().unwrap();
        init_bare_with_commit(remote_dir.path());
        let url = format!("file://{}", remote_dir.path().display());

        let base = TempDir::new().unwrap();
        let mut sandbox = Sandbox::create("test-task-3", base.path())
            .await
            .unwrap();

        sandbox.clone(&url, "main", None).await.unwrap();
        sandbox.create_branch("feature/test").unwrap();

        let repo = git2::Repository::open(&sandbox.repo_dir).unwrap();
        let head = repo.head().unwrap();
        let shorthand = head.shorthand().unwrap_or("");
        assert_eq!(shorthand, "feature/test");
    }

    #[tokio::test]
    async fn has_changes_returns_false_on_clean_repo() {
        let remote_dir = TempDir::new().unwrap();
        init_bare_with_commit(remote_dir.path());
        let url = format!("file://{}", remote_dir.path().display());

        let base = TempDir::new().unwrap();
        let mut sandbox = Sandbox::create("test-task-4", base.path())
            .await
            .unwrap();

        sandbox.clone(&url, "main", None).await.unwrap();

        assert!(!sandbox.has_changes().unwrap());
    }

    #[tokio::test]
    async fn has_changes_returns_true_after_writing_file() {
        let remote_dir = TempDir::new().unwrap();
        init_bare_with_commit(remote_dir.path());
        let url = format!("file://{}", remote_dir.path().display());

        let base = TempDir::new().unwrap();
        let mut sandbox = Sandbox::create("test-task-5", base.path())
            .await
            .unwrap();

        sandbox.clone(&url, "main", None).await.unwrap();

        std::fs::write(sandbox.repo_dir.join("newfile.txt"), "hello").unwrap();

        assert!(sandbox.has_changes().unwrap());
    }

    #[tokio::test]
    async fn commit_all_creates_commit_with_expected_message() {
        let remote_dir = TempDir::new().unwrap();
        init_bare_with_commit(remote_dir.path());
        let url = format!("file://{}", remote_dir.path().display());

        let base = TempDir::new().unwrap();
        let mut sandbox = Sandbox::create("test-task-6", base.path())
            .await
            .unwrap();

        sandbox.clone(&url, "main", None).await.unwrap();
        std::fs::write(sandbox.repo_dir.join("change.txt"), "content").unwrap();

        sandbox.commit_all("test commit message").unwrap();

        let repo = git2::Repository::open(&sandbox.repo_dir).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap_or(""), "test commit message");
    }

    #[tokio::test]
    async fn push_sends_commit_to_bare_remote() {
        let remote_dir = TempDir::new().unwrap();
        init_bare_with_commit(remote_dir.path());
        let url = format!("file://{}", remote_dir.path().display());

        let base = TempDir::new().unwrap();
        let mut sandbox = Sandbox::create("test-task-7", base.path())
            .await
            .unwrap();

        sandbox.clone(&url, "main", None).await.unwrap();
        sandbox.create_branch("feature/push-test").unwrap();
        std::fs::write(sandbox.repo_dir.join("pushed.txt"), "pushed content").unwrap();
        sandbox.commit_all("pushed commit").unwrap();

        sandbox.push("feature/push-test", None).await.unwrap();

        let bare = git2::Repository::open_bare(remote_dir.path()).unwrap();
        let branch_ref = bare.find_reference("refs/heads/feature/push-test");
        assert!(
            branch_ref.is_ok(),
            "Branch should exist in remote after push"
        );
    }

    #[tokio::test]
    async fn cleanup_removes_directory() {
        let base = TempDir::new().unwrap();
        let sandbox = Sandbox::create("test-task-8", base.path())
            .await
            .unwrap();

        let sandbox_dir = sandbox.sandbox_dir.clone();
        assert!(sandbox_dir.exists());

        sandbox.cleanup().unwrap();
        assert!(!sandbox_dir.exists());
    }
}

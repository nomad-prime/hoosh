use anyhow::{Context, Result, bail};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

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
        let mut cmd = tokio::process::Command::new("git");
        cmd.args([
            "clone",
            "--branch",
            base_branch,
            "--single-branch",
            repo_url,
            self.repo_dir.to_str().context("Invalid repo dir path")?,
        ]);

        if let Some(key) = ssh_key_path {
            cmd.env(
                "GIT_SSH_COMMAND",
                format!(
                    "ssh -i {} -o StrictHostKeyChecking=yes -o BatchMode=yes",
                    key.display()
                ),
            );
        }

        let output = tokio::time::timeout(std::time::Duration::from_secs(300), cmd.output())
            .await
            .context("Repository clone timed out after 300 seconds")?
            .context("Failed to spawn git clone")?;

        if output.status.success() {
            self.cloned = true;
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("git clone failed: {}", stderr.trim());
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

    fn init_repo_with_commit(path: &std::path::Path) {
        let run = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(path)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_AUTHOR_NAME", "Test")
                .env("GIT_AUTHOR_EMAIL", "test@example.com")
                .env("GIT_COMMITTER_NAME", "Test")
                .env("GIT_COMMITTER_EMAIL", "test@example.com")
                .output()
                .unwrap();
        };
        run(&["init", "-b", "main"]);
        run(&["commit", "--allow-empty", "-m", "Initial commit"]);
    }

    #[tokio::test]
    async fn create_makes_sandbox_directory() {
        let base = TempDir::new().unwrap();
        let sandbox = Sandbox::create("test-task-1", base.path()).await.unwrap();

        assert!(sandbox.repo_dir.exists());
        assert!(sandbox.sandbox_dir.exists());
    }

    #[tokio::test]
    async fn clone_creates_repo_at_sandbox_path() {
        let remote_dir = TempDir::new().unwrap();
        init_repo_with_commit(remote_dir.path());

        let base = TempDir::new().unwrap();
        let mut sandbox = Sandbox::create("test-task-2", base.path()).await.unwrap();

        sandbox
            .clone(remote_dir.path().to_str().unwrap(), "main", None)
            .await
            .unwrap();

        assert!(sandbox.repo_dir.join(".git").exists());
    }

    #[tokio::test]
    async fn cleanup_removes_directory() {
        let base = TempDir::new().unwrap();
        let sandbox = Sandbox::create("test-task-8", base.path()).await.unwrap();

        let sandbox_dir = sandbox.sandbox_dir.clone();
        assert!(sandbox_dir.exists());

        sandbox.cleanup().unwrap();
        assert!(!sandbox_dir.exists());
    }
}

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::config::AppConfig;
use crate::console::console;
use crate::daemon::job::{Job, JobStatus};

pub struct JobStore {
    tasks_dir: PathBuf,
    cache: Arc<RwLock<HashMap<String, Job>>>,
}

impl JobStore {
    pub fn new() -> Result<Self> {
        let tasks_dir = AppConfig::hoosh_data_dir()
            .context("Could not determine data directory")?
            .join("daemon")
            .join("tasks");
        std::fs::create_dir_all(&tasks_dir).with_context(|| {
            format!("Failed to create tasks directory: {}", tasks_dir.display())
        })?;

        let store = Self {
            tasks_dir,
            cache: Arc::new(RwLock::new(HashMap::new())),
        };

        store.recover_running_jobs()?;

        Ok(store)
    }

    pub fn new_with_dir(tasks_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&tasks_dir).with_context(|| {
            format!("Failed to create tasks directory: {}", tasks_dir.display())
        })?;

        let store = Self {
            tasks_dir,
            cache: Arc::new(RwLock::new(HashMap::new())),
        };

        store.recover_running_jobs()?;

        Ok(store)
    }

    fn recover_running_jobs(&self) -> Result<()> {
        let jobs = self.load_all_from_disk()?;
        let mut cache = self.cache.write().unwrap();
        for mut job in jobs {
            if job.status == JobStatus::Running {
                job.status = JobStatus::Failed;
                job.error_message = Some("[incomplete] daemon restarted unexpectedly".to_string());
                job.completed_at = Some(Utc::now());
                let path = self.task_path(&job.id);
                let json = serde_json::to_string_pretty(&job)?;
                std::fs::write(&path, json)?;
            }
            cache.insert(job.id.clone(), job);
        }
        Ok(())
    }

    fn task_path(&self, id: &str) -> PathBuf {
        self.tasks_dir.join(format!("{}.json", id))
    }

    fn load_all_from_disk(&self) -> Result<Vec<Job>> {
        let mut jobs = Vec::new();
        let entries = std::fs::read_dir(&self.tasks_dir).with_context(|| {
            format!(
                "Failed to read tasks directory: {}",
                self.tasks_dir.display()
            )
        })?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read job file: {}", path.display()))?;
                match serde_json::from_str::<Job>(&content) {
                    Ok(job) => jobs.push(job),
                    Err(e) => {
                        console().warning(&format!(
                            "Failed to parse job file {}: {}",
                            path.display(),
                            e
                        ));
                    }
                }
            }
        }

        Ok(jobs)
    }

    pub fn create(&self, job: &Job) -> Result<()> {
        let path = self.task_path(&job.id);
        let json = serde_json::to_string_pretty(job).context("Failed to serialize job")?;

        let tmp = tempfile::NamedTempFile::new_in(&self.tasks_dir)
            .context("Failed to create temp file for job")?;
        std::fs::write(tmp.path(), &json).context("Failed to write job to temp file")?;
        tmp.persist(&path)
            .with_context(|| format!("Failed to persist job file: {}", path.display()))?;

        let mut cache = self.cache.write().unwrap();
        cache.insert(job.id.clone(), job.clone());

        Ok(())
    }

    pub fn update(&self, job: &Job) -> Result<()> {
        let path = self.task_path(&job.id);
        let json = serde_json::to_string_pretty(job).context("Failed to serialize job")?;

        let tmp = tempfile::NamedTempFile::new_in(&self.tasks_dir)
            .context("Failed to create temp file for job update")?;
        std::fs::write(tmp.path(), &json).context("Failed to write job update to temp file")?;
        tmp.persist(&path)
            .with_context(|| format!("Failed to persist job update: {}", path.display()))?;

        let mut cache = self.cache.write().unwrap();
        cache.insert(job.id.clone(), job.clone());

        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<Job>> {
        {
            let cache = self.cache.read().unwrap();
            if let Some(job) = cache.get(id) {
                return Ok(Some(job.clone()));
            }
        }

        let path = self.task_path(id);
        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read job file: {}", path.display()))?;
        let job: Job = serde_json::from_str(&content).context("Failed to parse job")?;

        let mut cache = self.cache.write().unwrap();
        cache.insert(job.id.clone(), job.clone());

        Ok(Some(job))
    }

    pub fn load_all(&self) -> Result<Vec<Job>> {
        let cache = self.cache.read().unwrap();
        Ok(cache.values().cloned().collect())
    }

    pub fn query_active_by_trigger_ref(&self, trigger_ref: &str) -> Option<String> {
        let cache = self.cache.read().unwrap();
        cache.values().find_map(|job| {
            if matches!(job.status, JobStatus::Queued | JobStatus::Running)
                && let Some(ref trigger) = job.trigger
                && trigger.trigger_ref == trigger_ref
            {
                return Some(job.id.clone());
            }
            None
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::job::{GithubEventType, GithubTrigger, Job, JobStatus};
    use tempfile::TempDir;

    fn make_store(dir: &TempDir) -> JobStore {
        JobStore::new_with_dir(dir.path().join("tasks")).unwrap()
    }

    fn make_job() -> Job {
        Job::new(
            "https://github.com/example/repo".to_string(),
            "main".to_string(),
            "Do something".to_string(),
            None,
            100_000,
        )
    }

    fn make_trigger(trigger_ref: &str) -> GithubTrigger {
        GithubTrigger {
            event_type: GithubEventType::IssueComment,
            delivery_id: "delivery-1".to_string(),
            trigger_ref: trigger_ref.to_string(),
            repo_full_name: "owner/repo".to_string(),
            repo_url: "https://github.com/owner/repo.git".to_string(),
            default_branch: "main".to_string(),
            actor_login: "alice".to_string(),
            issue_or_pr_number: 47,
            comment_url: None,
            raw_payload: serde_json::Value::Null,
        }
    }

    #[test]
    fn create_and_load_persists_to_disk() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        let job = make_job();
        let job_id = job.id.clone();

        store.create(&job).unwrap();

        let path = store.task_path(&job_id);
        assert!(path.exists(), "Job file should exist on disk");

        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: Job = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.id, job_id);
    }

    #[test]
    fn update_changes_status() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        let mut job = make_job();
        let job_id = job.id.clone();

        store.create(&job).unwrap();

        job.status = JobStatus::Running;
        store.update(&job).unwrap();

        let loaded = store.get(&job_id).unwrap().unwrap();
        assert_eq!(loaded.status, JobStatus::Running);
    }

    #[test]
    fn load_all_returns_all_jobs() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);

        let job1 = make_job();
        let job2 = make_job();

        store.create(&job1).unwrap();
        store.create(&job2).unwrap();

        let all = store.load_all().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn queued_job_with_trigger_ref_returns_id() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        let mut job = make_job();
        job.trigger = Some(make_trigger("issue:47"));
        let job_id = job.id.clone();
        store.create(&job).unwrap();

        let found = store.query_active_by_trigger_ref("issue:47");
        assert_eq!(found, Some(job_id));
    }

    #[test]
    fn running_job_with_trigger_ref_returns_id() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        let mut job = make_job();
        job.status = JobStatus::Running;
        job.trigger = Some(make_trigger("pr:82"));
        let job_id = job.id.clone();
        store.create(&job).unwrap();

        let found = store.query_active_by_trigger_ref("pr:82");
        assert_eq!(found, Some(job_id));
    }

    #[test]
    fn completed_job_with_trigger_ref_returns_none() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        let mut job = make_job();
        job.status = JobStatus::Completed;
        job.trigger = Some(make_trigger("issue:10"));
        store.create(&job).unwrap();

        let found = store.query_active_by_trigger_ref("issue:10");
        assert_eq!(found, None);
    }

    #[test]
    fn failed_job_with_trigger_ref_returns_none() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        let mut job = make_job();
        job.status = JobStatus::Failed;
        job.trigger = Some(make_trigger("issue:10"));
        store.create(&job).unwrap();

        let found = store.query_active_by_trigger_ref("issue:10");
        assert_eq!(found, None);
    }

    #[test]
    fn empty_store_returns_none() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        assert_eq!(store.query_active_by_trigger_ref("issue:99"), None);
    }

    #[test]
    fn crash_recovery_marks_running_as_failed() {
        let dir = TempDir::new().unwrap();
        let tasks_dir = dir.path().join("tasks");

        {
            let store = JobStore::new_with_dir(tasks_dir.clone()).unwrap();
            let mut job = make_job();
            job.status = JobStatus::Running;
            store.create(&job).unwrap();
        }

        let store2 = JobStore::new_with_dir(tasks_dir).unwrap();
        let all = store2.load_all().unwrap();
        assert_eq!(all.len(), 1);
        let recovered = &all[0];
        assert_eq!(recovered.status, JobStatus::Failed);
        assert!(
            recovered
                .error_message
                .as_deref()
                .unwrap_or("")
                .contains("[incomplete]"),
            "Error message should contain [incomplete]"
        );
    }
}

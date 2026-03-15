use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::daemon::task::{Task, TaskStatus};

pub struct TaskStore {
    tasks_dir: PathBuf,
    cache: Arc<RwLock<HashMap<String, Task>>>,
}

impl TaskStore {
    pub fn new() -> Result<Self> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        let tasks_dir = home.join(".hoosh").join("daemon").join("tasks");
        std::fs::create_dir_all(&tasks_dir).with_context(|| {
            format!("Failed to create tasks directory: {}", tasks_dir.display())
        })?;

        let store = Self {
            tasks_dir,
            cache: Arc::new(RwLock::new(HashMap::new())),
        };

        store.recover_running_tasks()?;

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

        store.recover_running_tasks()?;

        Ok(store)
    }

    fn recover_running_tasks(&self) -> Result<()> {
        let tasks = self.load_all_from_disk()?;
        let mut cache = self.cache.write().unwrap();
        for mut task in tasks {
            if task.status == TaskStatus::Running {
                task.status = TaskStatus::Failed;
                task.error_message = Some("[incomplete] daemon restarted unexpectedly".to_string());
                task.completed_at = Some(Utc::now());
                let path = self.task_path(&task.id);
                let json = serde_json::to_string_pretty(&task)?;
                std::fs::write(&path, json)?;
            }
            cache.insert(task.id.clone(), task);
        }
        Ok(())
    }

    fn task_path(&self, id: &str) -> PathBuf {
        self.tasks_dir.join(format!("{}.json", id))
    }

    fn load_all_from_disk(&self) -> Result<Vec<Task>> {
        let mut tasks = Vec::new();
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
                    .with_context(|| format!("Failed to read task file: {}", path.display()))?;
                match serde_json::from_str::<Task>(&content) {
                    Ok(task) => tasks.push(task),
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to parse task file {}: {}",
                            path.display(),
                            e
                        );
                    }
                }
            }
        }

        Ok(tasks)
    }

    pub fn create(&self, task: &Task) -> Result<()> {
        let path = self.task_path(&task.id);
        let json = serde_json::to_string_pretty(task).context("Failed to serialize task")?;

        let tmp = tempfile::NamedTempFile::new_in(&self.tasks_dir)
            .context("Failed to create temp file for task")?;
        std::fs::write(tmp.path(), &json).context("Failed to write task to temp file")?;
        tmp.persist(&path)
            .with_context(|| format!("Failed to persist task file: {}", path.display()))?;

        let mut cache = self.cache.write().unwrap();
        cache.insert(task.id.clone(), task.clone());

        Ok(())
    }

    pub fn update(&self, task: &Task) -> Result<()> {
        let path = self.task_path(&task.id);
        let json = serde_json::to_string_pretty(task).context("Failed to serialize task")?;

        let tmp = tempfile::NamedTempFile::new_in(&self.tasks_dir)
            .context("Failed to create temp file for task update")?;
        std::fs::write(tmp.path(), &json).context("Failed to write task update to temp file")?;
        tmp.persist(&path)
            .with_context(|| format!("Failed to persist task update: {}", path.display()))?;

        let mut cache = self.cache.write().unwrap();
        cache.insert(task.id.clone(), task.clone());

        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<Task>> {
        {
            let cache = self.cache.read().unwrap();
            if let Some(task) = cache.get(id) {
                return Ok(Some(task.clone()));
            }
        }

        let path = self.task_path(id);
        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read task file: {}", path.display()))?;
        let task: Task = serde_json::from_str(&content).context("Failed to parse task")?;

        let mut cache = self.cache.write().unwrap();
        cache.insert(task.id.clone(), task.clone());

        Ok(Some(task))
    }

    pub fn load_all(&self) -> Result<Vec<Task>> {
        let cache = self.cache.read().unwrap();
        Ok(cache.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::task::{Task, TaskStatus};
    use tempfile::TempDir;

    fn make_store(dir: &TempDir) -> TaskStore {
        TaskStore::new_with_dir(dir.path().join("tasks")).unwrap()
    }

    fn make_task() -> Task {
        Task::new(
            "https://github.com/example/repo".to_string(),
            "main".to_string(),
            "Do something".to_string(),
            None,
            100_000,
        )
    }

    #[test]
    fn create_and_load_persists_to_disk() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        let task = make_task();
        let task_id = task.id.clone();

        store.create(&task).unwrap();

        let path = store.task_path(&task_id);
        assert!(path.exists(), "Task file should exist on disk");

        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: Task = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.id, task_id);
    }

    #[test]
    fn update_changes_status() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        let mut task = make_task();
        let task_id = task.id.clone();

        store.create(&task).unwrap();

        task.status = TaskStatus::Running;
        store.update(&task).unwrap();

        let loaded = store.get(&task_id).unwrap().unwrap();
        assert_eq!(loaded.status, TaskStatus::Running);
    }

    #[test]
    fn load_all_returns_all_tasks() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);

        let task1 = make_task();
        let task2 = make_task();

        store.create(&task1).unwrap();
        store.create(&task2).unwrap();

        let all = store.load_all().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn crash_recovery_marks_running_as_failed() {
        let dir = TempDir::new().unwrap();
        let tasks_dir = dir.path().join("tasks");

        {
            let store = TaskStore::new_with_dir(tasks_dir.clone()).unwrap();
            let mut task = make_task();
            task.status = TaskStatus::Running;
            store.create(&task).unwrap();
        }

        let store2 = TaskStore::new_with_dir(tasks_dir).unwrap();
        let all = store2.load_all().unwrap();
        assert_eq!(all.len(), 1);
        let recovered = &all[0];
        assert_eq!(recovered.status, TaskStatus::Failed);
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

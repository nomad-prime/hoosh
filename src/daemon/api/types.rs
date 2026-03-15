use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::daemon::task::TaskStatus;

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitTaskRequest {
    pub repo_url: String,
    pub base_branch: String,
    pub instructions: String,
    pub pr_title: Option<String>,
    #[serde(default)]
    pub pr_labels: Vec<String>,
    pub token_budget: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitTaskResponse {
    pub task_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskResponse {
    pub id: String,
    pub repo_url: String,
    pub base_branch: String,
    pub instructions: String,
    pub pr_title: Option<String>,
    pub pr_labels: Vec<String>,
    pub token_budget: usize,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub pr_url: Option<String>,
    pub branch: Option<String>,
    pub tokens_consumed: usize,
    pub error_message: Option<String>,
    pub sandbox_path: Option<PathBuf>,
    pub log_path: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub active_tasks: usize,
    pub shutting_down: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

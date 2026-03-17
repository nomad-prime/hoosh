use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GithubEventType {
    IssueComment,
    PullRequestReview,
    PullRequestReviewComment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubTrigger {
    pub event_type: GithubEventType,
    pub delivery_id: String,
    pub trigger_ref: String,
    pub repo_full_name: String,
    pub repo_url: String,
    pub default_branch: String,
    pub actor_login: String,
    pub issue_or_pr_number: u64,
    pub comment_url: Option<String>,
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
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
    #[serde(default)]
    pub trigger: Option<GithubTrigger>,
}

impl Task {
    pub fn new(
        repo_url: String,
        base_branch: String,
        instructions: String,
        token_budget: Option<usize>,
        default_budget: usize,
    ) -> Self {
        let id = format!("hoosh-{}", Uuid::new_v4());
        Self {
            id,
            repo_url,
            base_branch,
            instructions,
            pr_title: None,
            pr_labels: vec![],
            token_budget: token_budget.unwrap_or(default_budget),
            status: TaskStatus::Queued,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            pr_url: None,
            branch: None,
            tokens_consumed: 0,
            error_message: None,
            sandbox_path: None,
            log_path: None,
            trigger: None,
        }
    }
}

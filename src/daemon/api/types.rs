use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::daemon::job::{GithubEventType, GithubTrigger, JobStatus};

#[derive(Debug, Serialize, Deserialize)]
pub struct GithubTriggerResponse {
    pub event_type: GithubEventType,
    pub delivery_id: String,
    pub trigger_ref: String,
    pub repo_full_name: String,
    pub repo_url: String,
    pub default_branch: String,
    pub actor_login: String,
    pub issue_or_pr_number: u64,
    pub comment_url: Option<String>,
}

impl From<&GithubTrigger> for GithubTriggerResponse {
    fn from(t: &GithubTrigger) -> Self {
        Self {
            event_type: t.event_type.clone(),
            delivery_id: t.delivery_id.clone(),
            trigger_ref: t.trigger_ref.clone(),
            repo_full_name: t.repo_full_name.clone(),
            repo_url: t.repo_url.clone(),
            default_branch: t.default_branch.clone(),
            actor_login: t.actor_login.clone(),
            issue_or_pr_number: t.issue_or_pr_number,
            comment_url: t.comment_url.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitJobRequest {
    pub repo_url: String,
    pub base_branch: String,
    pub instructions: String,
    pub pr_title: Option<String>,
    #[serde(default)]
    pub pr_labels: Vec<String>,
    pub token_budget: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitJobResponse {
    pub job_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobResponse {
    pub id: String,
    pub repo_url: String,
    pub base_branch: String,
    pub instructions: String,
    pub pr_title: Option<String>,
    pub pr_labels: Vec<String>,
    pub token_budget: usize,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub pr_url: Option<String>,
    pub branch: Option<String>,
    pub tokens_consumed: usize,
    pub error_message: Option<String>,
    pub sandbox_path: Option<PathBuf>,
    pub log_path: Option<PathBuf>,
    pub trigger: Option<GithubTriggerResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub active_jobs: usize,
    pub shutting_down: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::job::GithubEventType;

    fn make_trigger() -> GithubTrigger {
        GithubTrigger {
            event_type: GithubEventType::IssueComment,
            delivery_id: "delivery-abc".to_string(),
            trigger_ref: "issue:47".to_string(),
            repo_full_name: "acme/backend".to_string(),
            repo_url: "https://github.com/acme/backend.git".to_string(),
            default_branch: "main".to_string(),
            actor_login: "alice".to_string(),
            issue_or_pr_number: 47,
            comment_url: Some("https://github.com/acme/backend/issues/47#comment-1".to_string()),
            raw_payload: serde_json::json!({"secret_field": "should-not-appear"}),
        }
    }

    #[test]
    fn task_response_trigger_contains_expected_fields() {
        let trigger = make_trigger();
        let response = GithubTriggerResponse::from(&trigger);
        let json = serde_json::to_value(&response).unwrap();

        assert_eq!(json["event_type"], "issue_comment");
        assert_eq!(json["trigger_ref"], "issue:47");
        assert_eq!(json["actor_login"], "alice");
    }

    #[test]
    fn task_response_trigger_omits_raw_payload() {
        let trigger = make_trigger();
        let response = GithubTriggerResponse::from(&trigger);
        let json = serde_json::to_value(&response).unwrap();

        assert!(
            json.get("raw_payload").is_none(),
            "raw_payload must not appear in API response"
        );
    }
}

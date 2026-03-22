use anyhow::Result;
use serde::Deserialize;

use crate::daemon::job::{GithubEventType, GithubTrigger};

// ---- Internal deserialization structs ----

#[derive(Deserialize)]
pub(crate) struct IssueCommentPayload {
    pub comment: CommentBody,
    pub issue: IssueRef,
    pub repository: RepoRef,
    pub sender: ActorRef,
}

#[derive(Deserialize)]
pub(crate) struct CommentBody {
    pub body: String,
    pub html_url: String,
}

#[derive(Deserialize)]
pub(crate) struct IssueRef {
    pub number: u64,
    pub pull_request: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub(crate) struct RepoRef {
    pub full_name: String,
    pub clone_url: String,
    pub default_branch: String,
}

#[derive(Deserialize)]
pub(crate) struct ActorRef {
    pub login: String,
}

#[derive(Deserialize)]
pub(crate) struct PullRequestReviewPayload {
    pub review: ReviewBody,
    pub pull_request: PrRef,
    pub repository: RepoRef,
    pub sender: ActorRef,
}

#[derive(Deserialize)]
pub(crate) struct ReviewBody {
    pub body: Option<String>,
    pub html_url: String,
}

#[derive(Deserialize)]
pub(crate) struct PrRef {
    pub number: u64,
}

#[derive(Deserialize)]
pub(crate) struct PullRequestReviewCommentPayload {
    pub comment: ReviewCommentBody,
    pub pull_request: PrRef,
    pub repository: RepoRef,
    pub sender: ActorRef,
}

#[derive(Deserialize)]
pub(crate) struct ReviewCommentBody {
    pub body: String,
    pub html_url: String,
}

// ---- Public API ----

fn to_ssh_url(https_url: &str) -> String {
    if let Some(rest) = https_url.strip_prefix("https://github.com/") {
        format!("git@github.com:{}", rest)
    } else {
        https_url.to_string()
    }
}

pub fn mentions_handle(body: &str, handle: &str) -> bool {
    let escaped = regex::escape(handle);
    let pattern = format!(r"(?i){}(?:[^a-zA-Z0-9\-]|$)", escaped);
    regex::Regex::new(&pattern)
        .map(|re| re.is_match(body))
        .unwrap_or(false)
}

pub fn is_bot_sender(login: &str, bot_login: Option<&str>) -> bool {
    match bot_login {
        Some(bot) => login == bot,
        None => false,
    }
}

pub fn parse_github_event(
    event_type: &str,
    payload: &[u8],
    mention_handle: &str,
    bot_login: Option<&str>,
    delivery_id: &str,
) -> Result<Option<GithubTrigger>> {
    let raw_payload: serde_json::Value = serde_json::from_slice(payload)?;

    match event_type {
        "issue_comment" => {
            if raw_payload["action"].as_str() != Some("created") {
                return Ok(None);
            }
            let p: IssueCommentPayload = serde_json::from_value(raw_payload.clone())?;
            if is_bot_sender(&p.sender.login, bot_login) {
                return Ok(None);
            }
            if !mentions_handle(&p.comment.body, mention_handle) {
                return Ok(None);
            }
            let trigger_ref = if p.issue.pull_request.is_some() {
                format!("pr:{}", p.issue.number)
            } else {
                format!("issue:{}", p.issue.number)
            };
            Ok(Some(GithubTrigger {
                event_type: GithubEventType::IssueComment,
                delivery_id: delivery_id.to_string(),
                trigger_ref,
                repo_full_name: p.repository.full_name,
                repo_url: to_ssh_url(&p.repository.clone_url),
                default_branch: p.repository.default_branch,
                actor_login: p.sender.login,
                issue_or_pr_number: p.issue.number,
                comment_url: Some(p.comment.html_url),
                raw_payload,
            }))
        }
        "pull_request_review" => {
            if raw_payload["action"].as_str() != Some("submitted") {
                return Ok(None);
            }
            let p: PullRequestReviewPayload = serde_json::from_value(raw_payload.clone())?;
            if is_bot_sender(&p.sender.login, bot_login) {
                return Ok(None);
            }
            let body = p.review.body.as_deref().unwrap_or("");
            if !mentions_handle(body, mention_handle) {
                return Ok(None);
            }
            Ok(Some(GithubTrigger {
                event_type: GithubEventType::PullRequestReview,
                delivery_id: delivery_id.to_string(),
                trigger_ref: format!("pr:{}", p.pull_request.number),
                repo_full_name: p.repository.full_name,
                repo_url: to_ssh_url(&p.repository.clone_url),
                default_branch: p.repository.default_branch,
                actor_login: p.sender.login,
                issue_or_pr_number: p.pull_request.number,
                comment_url: Some(p.review.html_url),
                raw_payload,
            }))
        }
        "pull_request_review_comment" => {
            if raw_payload["action"].as_str() != Some("created") {
                return Ok(None);
            }
            let p: PullRequestReviewCommentPayload = serde_json::from_value(raw_payload.clone())?;
            if is_bot_sender(&p.sender.login, bot_login) {
                return Ok(None);
            }
            if !mentions_handle(&p.comment.body, mention_handle) {
                return Ok(None);
            }
            Ok(Some(GithubTrigger {
                event_type: GithubEventType::PullRequestReviewComment,
                delivery_id: delivery_id.to_string(),
                trigger_ref: format!("pr:{}", p.pull_request.number),
                repo_full_name: p.repository.full_name,
                repo_url: to_ssh_url(&p.repository.clone_url),
                default_branch: p.repository.default_branch,
                actor_login: p.sender.login,
                issue_or_pr_number: p.pull_request.number,
                comment_url: Some(p.comment.html_url),
                raw_payload,
            }))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- mentions_handle tests (T010) ----

    #[test]
    fn mentions_handle_exact_match_returns_true() {
        assert!(mentions_handle("hey @hoosh please fix this", "@hoosh"));
    }

    #[test]
    fn mentions_handle_absent_returns_false() {
        assert!(!mentions_handle("hey please fix this", "@hoosh"));
    }

    #[test]
    fn mentions_handle_case_insensitive_match() {
        assert!(mentions_handle("hey @Hoosh please fix this", "@hoosh"));
    }

    #[test]
    fn mentions_handle_prefix_of_longer_handle_returns_false() {
        assert!(!mentions_handle("hey @hoosh-bot can you help?", "@hoosh"));
    }

    #[test]
    fn mentions_handle_alphanumeric_suffix_returns_false() {
        assert!(!mentions_handle("@hoosh2 please look at this", "@hoosh"));
    }

    #[test]
    fn mentions_handle_at_end_of_string_returns_true() {
        assert!(mentions_handle("@hoosh", "@hoosh"));
    }

    #[test]
    fn mentions_handle_empty_body_returns_false() {
        assert!(!mentions_handle("", "@hoosh"));
    }

    #[test]
    fn mentions_handle_followed_by_comma_returns_true() {
        assert!(mentions_handle("@hoosh, please review", "@hoosh"));
    }

    #[test]
    fn mentions_handle_followed_by_period_returns_true() {
        assert!(mentions_handle("Thanks @hoosh.", "@hoosh"));
    }

    #[test]
    fn mentions_handle_followed_by_exclamation_returns_true() {
        assert!(mentions_handle("cc @hoosh!", "@hoosh"));
    }

    #[test]
    fn mentions_handle_followed_by_colon_returns_true() {
        assert!(mentions_handle("@hoosh: please check", "@hoosh"));
    }

    #[test]
    fn mentions_handle_hyphenated_configured_handle_exact_match() {
        assert!(mentions_handle("@hoosh-ci please run", "@hoosh-ci"));
    }

    #[test]
    fn mentions_handle_hyphenated_configured_handle_prefix_returns_false() {
        assert!(!mentions_handle("@hoosh-ci-staging", "@hoosh-ci"));
    }

    #[test]
    fn mentions_handle_hyphenated_configured_handle_at_end_of_string() {
        assert!(mentions_handle("@hoosh-ci", "@hoosh-ci"));
    }

    #[test]
    fn mentions_handle_mixed_exact_and_longer_in_body_returns_true() {
        assert!(mentions_handle("@hoosh @hoosh-bot @hoosh please help", "@hoosh"));
    }

    // ---- is_bot_sender tests (T010) ----

    #[test]
    fn bot_login_matches_sender_returns_true() {
        assert!(is_bot_sender("hoosh-bot", Some("hoosh-bot")));
    }

    #[test]
    fn bot_login_does_not_match_sender_returns_false() {
        assert!(!is_bot_sender("alice", Some("hoosh-bot")));
    }

    #[test]
    fn bot_login_none_always_returns_false() {
        assert!(!is_bot_sender("hoosh-bot", None));
    }

    // ---- parse_github_event tests (T011) ----

    fn issue_comment_payload(action: &str, body: &str, sender: &str, is_pr: bool) -> Vec<u8> {
        let pull_request = if is_pr {
            serde_json::json!({"url": "https://api.github.com/repos/owner/repo/pulls/5"})
        } else {
            serde_json::Value::Null
        };
        let payload = serde_json::json!({
            "action": action,
            "comment": {
                "body": body,
                "html_url": "https://github.com/owner/repo/issues/47#issuecomment-1"
            },
            "issue": {
                "number": 47,
                "pull_request": if is_pr { pull_request } else { serde_json::Value::Null }
            },
            "repository": {
                "full_name": "owner/repo",
                "clone_url": "https://github.com/owner/repo.git",
                "default_branch": "main"
            },
            "sender": {
                "login": sender
            }
        });
        serde_json::to_vec(&payload).unwrap()
    }

    fn pr_review_payload(action: &str, body: &str, sender: &str) -> Vec<u8> {
        let payload = serde_json::json!({
            "action": action,
            "review": {
                "body": body,
                "state": "COMMENTED",
                "html_url": "https://github.com/owner/repo/pull/82#pullrequestreview-1"
            },
            "pull_request": {
                "number": 82
            },
            "repository": {
                "full_name": "owner/repo",
                "clone_url": "https://github.com/owner/repo.git",
                "default_branch": "develop"
            },
            "sender": {
                "login": sender
            }
        });
        serde_json::to_vec(&payload).unwrap()
    }

    fn pr_review_comment_payload(action: &str, body: &str, sender: &str) -> Vec<u8> {
        let payload = serde_json::json!({
            "action": action,
            "comment": {
                "body": body,
                "html_url": "https://github.com/owner/repo/pull/82#discussion_r1"
            },
            "pull_request": {
                "number": 82
            },
            "repository": {
                "full_name": "owner/repo",
                "clone_url": "https://github.com/owner/repo.git",
                "default_branch": "main"
            },
            "sender": {
                "login": sender
            }
        });
        serde_json::to_vec(&payload).unwrap()
    }

    #[test]
    fn issue_comment_with_mention_returns_trigger() {
        let payload = issue_comment_payload("created", "@hoosh fix this", "alice", false);
        let trigger = parse_github_event("issue_comment", &payload, "@hoosh", None, "delivery-1")
            .unwrap()
            .unwrap();
        assert_eq!(trigger.trigger_ref, "issue:47");
        assert_eq!(trigger.actor_login, "alice");
        assert_eq!(trigger.default_branch, "main");
        assert_eq!(trigger.repo_url, "git@github.com:owner/repo.git");
        assert!(trigger.raw_payload.is_object());
    }

    #[test]
    fn issue_comment_on_pr_thread_uses_pr_trigger_ref() {
        let payload = issue_comment_payload("created", "@hoosh fix this", "alice", true);
        let trigger = parse_github_event("issue_comment", &payload, "@hoosh", None, "delivery-1")
            .unwrap()
            .unwrap();
        assert_eq!(trigger.trigger_ref, "pr:47");
    }

    #[test]
    fn issue_comment_without_mention_returns_none() {
        let payload = issue_comment_payload("created", "just a normal comment", "alice", false);
        let result =
            parse_github_event("issue_comment", &payload, "@hoosh", None, "delivery-1").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn issue_comment_unsupported_action_returns_none() {
        let payload = issue_comment_payload("edited", "@hoosh fix this", "alice", false);
        let result =
            parse_github_event("issue_comment", &payload, "@hoosh", None, "delivery-1").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn issue_comment_from_bot_returns_none() {
        let payload = issue_comment_payload("created", "@hoosh fix this", "hoosh-bot", false);
        let result = parse_github_event(
            "issue_comment",
            &payload,
            "@hoosh",
            Some("hoosh-bot"),
            "delivery-1",
        )
        .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn pr_review_with_mention_returns_trigger() {
        let payload = pr_review_payload("submitted", "@hoosh fix this", "alice");
        let trigger = parse_github_event(
            "pull_request_review",
            &payload,
            "@hoosh",
            None,
            "delivery-2",
        )
        .unwrap()
        .unwrap();
        assert_eq!(trigger.trigger_ref, "pr:82");
        assert_eq!(trigger.actor_login, "alice");
        assert_eq!(trigger.default_branch, "develop");
    }

    #[test]
    fn pr_review_comment_with_mention_returns_trigger() {
        let payload = pr_review_comment_payload("created", "@hoosh check this line", "bob");
        let trigger = parse_github_event(
            "pull_request_review_comment",
            &payload,
            "@hoosh",
            None,
            "delivery-3",
        )
        .unwrap()
        .unwrap();
        assert_eq!(trigger.trigger_ref, "pr:82");
        assert_eq!(trigger.actor_login, "bob");
    }

    #[test]
    fn unsupported_event_type_returns_none() {
        let payload = b"{}";
        let result = parse_github_event("push", payload, "@hoosh", None, "delivery-1").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn invalid_json_returns_err() {
        let result = parse_github_event("issue_comment", b"not-json", "@hoosh", None, "d-1");
        assert!(result.is_err());
    }
}

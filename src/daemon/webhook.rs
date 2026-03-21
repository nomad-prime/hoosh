use crate::daemon::api::AppState;
use crate::daemon::github_event::parse_github_event;
use crate::daemon::task::Task;
use axum::{
    Json,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub fn verify_signature(secret: &str, body: &[u8], signature_header: &str) -> bool {
    let hex_sig = match signature_header.strip_prefix("sha256=") {
        Some(s) => s,
        None => return false,
    };
    let sig_bytes = match hex::decode(hex_sig) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(body);
    mac.verify_slice(&sig_bytes).is_ok()
}

pub async fn handle_github_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let secret = match &state.config.github.webhook_secret {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "not_configured",
                    "detail": "github.webhook_secret is not set in daemon config"
                })),
            )
                .into_response();
        }
    };

    let event_type = match headers.get("X-GitHub-Event").and_then(|v| v.to_str().ok()) {
        Some(e) => e.to_string(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "missing X-GitHub-Event header"})),
            )
                .into_response();
        }
    };

    let delivery_id = headers
        .get("X-GitHub-Delivery")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let signature = match headers
        .get("X-Hub-Signature-256")
        .and_then(|v| v.to_str().ok())
    {
        Some(s) => s.to_string(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "invalid_signature"})),
            )
                .into_response();
        }
    };

    if !verify_signature(&secret, &body, &signature) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "invalid_signature"})),
        )
            .into_response();
    }

    if state
        .shutting_down
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "daemon is shutting down"})),
        )
            .into_response();
    }

    let mention_handle = state.config.github.mention_handle.clone();

    let trigger = match parse_github_event(
        &event_type,
        &body,
        &mention_handle,
        state.config.github.bot_login.as_deref(),
        &delivery_id,
    ) {
        Ok(Some(t)) => t,
        Ok(None) => {
            return (
                StatusCode::OK,
                Json(serde_json::json!({"status": "no_action", "reason": "no_mention"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "error": "invalid_payload",
                    "detail": e.to_string()
                })),
            )
                .into_response();
        }
    };

    if let Some(existing_id) = state
        .store
        .query_active_by_trigger_ref(&trigger.trigger_ref)
    {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "no_action",
                "reason": "duplicate",
                "existing_task_id": existing_id
            })),
        )
            .into_response();
    }

    let pretty_json =
        serde_json::to_string_pretty(&trigger.raw_payload).unwrap_or_else(|_| "{}".to_string());
    let agent_message = format!(
        "You have been mentioned in a GitHub {event_type} event. The repository is already cloned at your working directory.\n\n<event>\n{pretty_json}\n</event>\n\nUse `gh` CLI and git for all GitHub operations. Determine the appropriate branch strategy from the event context, make your changes, and push. Do not wait for further input."
    );

    let mut task = Task::new(
        trigger.repo_url.clone(),
        trigger.default_branch.clone(),
        agent_message,
        None,
        state.config.default_token_budget,
    );
    task.trigger = Some(trigger);

    let task_id = task.id.clone();

    if let Err(e) = state.store.create(&task) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to create task: {}", e)})),
        )
            .into_response();
    }

    state.spawn_task(task_id.clone()).await;

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"status": "accepted", "task_id": task_id})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    fn make_signature(secret: &str, body: &[u8]) -> String {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key size");
        mac.update(body);
        let result = mac.finalize();
        format!("sha256={}", hex::encode(result.into_bytes()))
    }

    // ---- verify_signature tests (T009) ----

    #[test]
    fn valid_hmac_is_accepted() {
        let secret = "my-webhook-secret";
        let body = b"hello world";
        let sig = make_signature(secret, body);
        assert!(verify_signature(secret, body, &sig));
    }

    #[test]
    fn tampered_body_is_rejected() {
        let secret = "my-webhook-secret";
        let body = b"hello world";
        let sig = make_signature(secret, body);
        assert!(!verify_signature(secret, b"tampered body", &sig));
    }

    #[test]
    fn missing_sha256_prefix_returns_false() {
        assert!(!verify_signature("secret", b"body", "abc123"));
    }

    #[test]
    fn malformed_hex_in_signature_returns_false() {
        assert!(!verify_signature(
            "secret",
            b"body",
            "sha256=not-valid-hex!"
        ));
    }

    #[test]
    fn wrong_secret_is_rejected() {
        let body = b"hello world";
        let sig = make_signature("correct-secret", body);
        assert!(!verify_signature("wrong-secret", body, &sig));
    }
}

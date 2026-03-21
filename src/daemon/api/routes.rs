use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use std::sync::atomic::Ordering;

use crate::daemon::api::AppState;
use crate::daemon::api::types::{
    ErrorResponse, GithubTriggerResponse, HealthResponse, SubmitTaskRequest, SubmitTaskResponse,
    TaskResponse,
};
use crate::daemon::task::{Task, TaskStatus};

impl From<Task> for TaskResponse {
    fn from(t: Task) -> Self {
        let trigger = t.trigger.as_ref().map(GithubTriggerResponse::from);
        Self {
            id: t.id,
            repo_url: t.repo_url,
            base_branch: t.base_branch,
            instructions: t.instructions,
            pr_title: t.pr_title,
            pr_labels: t.pr_labels,
            token_budget: t.token_budget,
            status: t.status,
            created_at: t.created_at,
            started_at: t.started_at,
            completed_at: t.completed_at,
            pr_url: t.pr_url,
            branch: t.branch,
            tokens_consumed: t.tokens_consumed,
            error_message: t.error_message,
            sandbox_path: t.sandbox_path,
            log_path: t.log_path,
            trigger,
        }
    }
}

pub async fn submit_task(
    State(state): State<AppState>,
    Json(req): Json<SubmitTaskRequest>,
) -> impl IntoResponse {
    if state.shutting_down.load(Ordering::Relaxed) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "daemon is shutting down"})),
        )
            .into_response();
    }

    if req.repo_url.trim().is_empty()
        || req.base_branch.trim().is_empty()
        || req.instructions.trim().is_empty()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "repo_url, base_branch, and instructions are required"})),
        )
            .into_response();
    }

    let task = Task::new(
        req.repo_url,
        req.base_branch,
        req.instructions,
        req.token_budget,
        state.config.default_token_budget,
    );

    let mut task = task;
    task.pr_title = req.pr_title;
    task.pr_labels = req.pr_labels;

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
        Json(serde_json::to_value(SubmitTaskResponse { task_id }).unwrap()),
    )
        .into_response()
}

pub async fn list_tasks(State(state): State<AppState>) -> impl IntoResponse {
    match state.store.load_all() {
        Ok(tasks) => {
            let responses: Vec<TaskResponse> = tasks.into_iter().map(TaskResponse::from).collect();
            Json(responses).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn get_task(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.store.get(&id) {
        Ok(Some(task)) => Json(TaskResponse::from(task)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Task not found: {}", id),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn cancel_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let task = match state.store.get(&id) {
        Ok(Some(t)) => t,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Task not found: {}", id),
                }),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response();
        }
    };

    if task.status.is_terminal() {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "Task {} is already in terminal state: {:?}",
                    id, task.status
                ),
            }),
        )
            .into_response();
    }

    let active = state.active_tasks.read().await;
    if let Some((_, cancel)) = active.get(&id) {
        cancel.store(true, Ordering::Relaxed);
    }
    drop(active);

    let mut updated_task = task;
    updated_task.status = TaskStatus::Cancelled;
    if let Err(e) = state.store.update(&updated_task) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response();
    }

    StatusCode::NO_CONTENT.into_response()
}

pub async fn get_task_logs(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let task = match state.store.get(&id) {
        Ok(Some(t)) => t,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Task not found: {}", id),
                }),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response();
        }
    };

    let Some(log_path) = task.log_path else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Log file not yet available".to_string(),
            }),
        )
            .into_response();
    };

    match std::fs::read_to_string(&log_path) {
        Ok(content) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                "text/plain; charset=utf-8".parse().unwrap(),
            );
            (StatusCode::OK, headers, content).into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Log file not found".to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let uptime_seconds = state.uptime_start.elapsed().as_secs();

    let active_tasks = {
        let tasks = state.active_tasks.read().await;
        tasks.len()
    };

    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds,
        active_tasks,
        shutting_down: state.shutting_down.load(Ordering::Relaxed),
    })
    .into_response()
}

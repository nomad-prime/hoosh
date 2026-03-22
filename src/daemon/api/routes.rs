use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use std::sync::atomic::Ordering;

use crate::daemon::api::AppState;
use crate::daemon::api::types::{
    ErrorResponse, GithubTriggerResponse, HealthResponse, JobResponse, SubmitJobRequest,
    SubmitJobResponse,
};
use crate::daemon::job::{Job, JobStatus};

impl From<Job> for JobResponse {
    fn from(j: Job) -> Self {
        let trigger = j.trigger.as_ref().map(GithubTriggerResponse::from);
        Self {
            id: j.id,
            repo_url: j.repo_url,
            base_branch: j.base_branch,
            instructions: j.instructions,
            pr_title: j.pr_title,
            pr_labels: j.pr_labels,
            token_budget: j.token_budget,
            status: j.status,
            created_at: j.created_at,
            started_at: j.started_at,
            completed_at: j.completed_at,
            pr_url: j.pr_url,
            branch: j.branch,
            tokens_consumed: j.tokens_consumed,
            error_message: j.error_message,
            sandbox_path: j.sandbox_path,
            log_path: j.log_path,
            trigger,
        }
    }
}

pub async fn submit_job(
    State(state): State<AppState>,
    Json(req): Json<SubmitJobRequest>,
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

    let job = Job::new(
        req.repo_url,
        req.base_branch,
        req.instructions,
        req.token_budget,
        state.config.default_token_budget,
    );

    let mut job = job;
    job.pr_title = req.pr_title;
    job.pr_labels = req.pr_labels;

    let job_id = job.id.clone();

    if let Err(e) = state.store.create(&job) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to create job: {}", e)})),
        )
            .into_response();
    }

    state.spawn_job(job_id.clone()).await;

    (
        StatusCode::ACCEPTED,
        Json(serde_json::to_value(SubmitJobResponse { job_id }).unwrap()),
    )
        .into_response()
}

pub async fn list_jobs(State(state): State<AppState>) -> impl IntoResponse {
    match state.store.load_all() {
        Ok(jobs) => {
            let responses: Vec<JobResponse> = jobs.into_iter().map(JobResponse::from).collect();
            Json(responses).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn get_job(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.store.get(&id) {
        Ok(Some(job)) => Json(JobResponse::from(job)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Job not found: {}", id),
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

pub async fn cancel_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let job = match state.store.get(&id) {
        Ok(Some(j)) => j,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Job not found: {}", id),
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

    if job.status.is_terminal() {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("Job {} is already in terminal state: {:?}", id, job.status),
            }),
        )
            .into_response();
    }

    let active = state.active_jobs.read().await;
    if let Some((_, cancel)) = active.get(&id) {
        cancel.store(true, Ordering::Relaxed);
    }
    drop(active);

    let mut updated_job = job;
    updated_job.status = JobStatus::Cancelled;
    if let Err(e) = state.store.update(&updated_job) {
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

pub async fn get_job_logs(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let job = match state.store.get(&id) {
        Ok(Some(j)) => j,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Job not found: {}", id),
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

    let Some(log_path) = job.log_path else {
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

    let active_jobs = {
        let jobs = state.active_jobs.read().await;
        jobs.len()
    };

    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds,
        active_jobs,
        shutting_down: state.shutting_down.load(Ordering::Relaxed),
    })
    .into_response()
}

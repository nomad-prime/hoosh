pub mod routes;
pub mod types;

use anyhow::{Context, Result};
use axum::{Router, routing::get};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::config::AppConfig;
use crate::console::console;
use crate::daemon::config::DaemonConfig;
use crate::daemon::job::JobStatus;
use crate::daemon::job_executor::JobExecutor;
use crate::daemon::job_store::JobStore;

pub type ActiveJobMap = Arc<RwLock<HashMap<String, (JoinHandle<()>, Arc<AtomicBool>)>>>;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<JobStore>,
    pub executor: Arc<JobExecutor>,
    pub config: Arc<DaemonConfig>,
    pub active_jobs: ActiveJobMap,
    pub uptime_start: Instant,
    pub shutting_down: Arc<AtomicBool>,
}

impl AppState {
    pub async fn spawn_job(&self, job_id: String) {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel);
        let executor = Arc::clone(&self.executor);
        let id_for_spawn = job_id.clone();

        let active_jobs = Arc::clone(&self.active_jobs);
        let id_for_cleanup = job_id.clone();

        let handle = tokio::spawn(async move {
            executor.run(id_for_spawn, cancel_clone).await;
            active_jobs.write().await.remove(&id_for_cleanup);
        });

        self.active_jobs
            .write()
            .await
            .insert(job_id, (handle, cancel));
    }
}

pub struct DaemonServer {
    pub store: Arc<JobStore>,
    pub executor: Arc<JobExecutor>,
    pub config: Arc<DaemonConfig>,
    pub active_jobs: ActiveJobMap,
    pub uptime_start: Instant,
    pub shutting_down: Arc<AtomicBool>,
}

impl DaemonServer {
    pub fn new(
        config: Arc<DaemonConfig>,
        store: Arc<JobStore>,
        executor: Arc<JobExecutor>,
    ) -> Self {
        Self {
            store,
            executor,
            config,
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
            uptime_start: Instant::now(),
            shutting_down: Arc::new(AtomicBool::new(false)),
        }
    }

    fn app_state(&self) -> AppState {
        AppState {
            store: Arc::clone(&self.store),
            executor: Arc::clone(&self.executor),
            config: Arc::clone(&self.config),
            active_jobs: Arc::clone(&self.active_jobs),
            uptime_start: self.uptime_start,
            shutting_down: Arc::clone(&self.shutting_down),
        }
    }

    pub fn router(&self) -> Router {
        use axum::routing::post;
        use routes::*;

        Router::new()
            .route("/jobs", post(submit_job).get(list_jobs))
            .route("/jobs/:id", get(get_job).delete(cancel_job))
            .route("/jobs/:id/logs", get(get_job_logs))
            .route("/health", get(health))
            .route(
                "/github/webhook",
                post(crate::daemon::webhook::handle_github_webhook),
            )
            .with_state(self.app_state())
    }

    pub async fn shutdown(&self, force: bool) {
        self.shutting_down.store(true, Ordering::Relaxed);

        if force {
            let jobs = self.active_jobs.read().await;
            for (_, (_, cancel)) in jobs.iter() {
                cancel.store(true, Ordering::Relaxed);
            }
        }

        let mut jobs = self.active_jobs.write().await;
        for (_, (handle, _)) in jobs.drain() {
            let _ = handle.await;
        }
    }

    pub async fn start(self) -> Result<()> {
        let all_jobs = self.store.load_all().context("Failed to load jobs")?;
        for mut job in all_jobs {
            if job.status == JobStatus::Running {
                job.status = JobStatus::Failed;
                job.error_message = Some("[incomplete] daemon restarted unexpectedly".to_string());
                job.completed_at = Some(Utc::now());
                let _ = self.store.update(&job);
            }
        }

        // Write PID file
        if let Ok(data_dir) = AppConfig::hoosh_data_dir() {
            let _ = std::fs::create_dir_all(&data_dir);
            let pid_path = data_dir.join("daemon.pid");
            let pid = std::process::id();
            let _ = std::fs::write(&pid_path, pid.to_string());
        }

        let addr = self.config.bind_address;
        let router = self.router();

        let shutting_down = Arc::clone(&self.shutting_down);
        let active_jobs = Arc::clone(&self.active_jobs);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .with_context(|| format!("Failed to bind to {}", addr))?;

        console().plain(&format!("hoosh daemon listening on {}", addr));

        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let ctrl_c = tokio::signal::ctrl_c();

                #[cfg(unix)]
                {
                    use tokio::signal::unix::{SignalKind, signal};
                    let mut sigterm =
                        signal(SignalKind::terminate()).expect("Failed to install SIGTERM handler");

                    tokio::select! {
                        _ = ctrl_c => {}
                        _ = sigterm.recv() => {}
                    }
                }

                #[cfg(not(unix))]
                {
                    let _ = ctrl_c.await;
                }

                console().plain("hoosh daemon shutting down...");
                shutting_down.store(true, Ordering::Relaxed);

                let mut jobs = active_jobs.write().await;
                for (_, (handle, _)) in jobs.drain() {
                    let _ = handle.await;
                }

                if let Ok(data_dir) = AppConfig::hoosh_data_dir() {
                    let _ = std::fs::remove_file(data_dir.join("daemon.pid"));
                }
            })
            .await
            .context("Server error")?;

        Ok(())
    }
}

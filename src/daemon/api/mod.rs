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

use crate::daemon::config::DaemonConfig;
use crate::daemon::executor::TaskExecutor;
use crate::daemon::store::TaskStore;
use crate::daemon::task::TaskStatus;

pub type ActiveTaskMap = Arc<RwLock<HashMap<String, (JoinHandle<()>, Arc<AtomicBool>)>>>;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<TaskStore>,
    pub executor: Arc<TaskExecutor>,
    pub config: Arc<DaemonConfig>,
    pub active_tasks: ActiveTaskMap,
    pub uptime_start: Instant,
    pub shutting_down: Arc<AtomicBool>,
}

pub struct DaemonServer {
    pub store: Arc<TaskStore>,
    pub executor: Arc<TaskExecutor>,
    pub config: Arc<DaemonConfig>,
    pub active_tasks: ActiveTaskMap,
    pub uptime_start: Instant,
    pub shutting_down: Arc<AtomicBool>,
}

impl DaemonServer {
    pub fn new(
        config: Arc<DaemonConfig>,
        store: Arc<TaskStore>,
        executor: Arc<TaskExecutor>,
    ) -> Self {
        Self {
            store,
            executor,
            config,
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            uptime_start: Instant::now(),
            shutting_down: Arc::new(AtomicBool::new(false)),
        }
    }

    fn app_state(&self) -> AppState {
        AppState {
            store: Arc::clone(&self.store),
            executor: Arc::clone(&self.executor),
            config: Arc::clone(&self.config),
            active_tasks: Arc::clone(&self.active_tasks),
            uptime_start: self.uptime_start,
            shutting_down: Arc::clone(&self.shutting_down),
        }
    }

    pub fn router(&self) -> Router {
        use axum::routing::post;
        use routes::*;

        Router::new()
            .route("/tasks", post(submit_task).get(list_tasks))
            .route("/tasks/:id", get(get_task).delete(cancel_task))
            .route("/tasks/:id/logs", get(get_task_logs))
            .route("/health", get(health))
            .with_state(self.app_state())
    }

    pub async fn shutdown(&self, force: bool) {
        self.shutting_down.store(true, Ordering::Relaxed);

        if force {
            let tasks = self.active_tasks.read().await;
            for (_, (_, cancel)) in tasks.iter() {
                cancel.store(true, Ordering::Relaxed);
            }
        }

        let mut tasks = self.active_tasks.write().await;
        for (_, (handle, _)) in tasks.drain() {
            let _ = handle.await;
        }
    }

    pub async fn start(self) -> Result<()> {
        // Recover running tasks from a previous crash
        let all_tasks = self.store.load_all().context("Failed to load tasks")?;
        for mut task in all_tasks {
            if task.status == TaskStatus::Running {
                task.status = TaskStatus::Failed;
                task.error_message = Some("[incomplete] daemon restarted unexpectedly".to_string());
                task.completed_at = Some(Utc::now());
                let _ = self.store.update(&task);
            }
        }

        // Write PID file
        if let Some(home) = dirs::home_dir() {
            let hoosh_dir = home.join(".hoosh");
            let _ = std::fs::create_dir_all(&hoosh_dir);
            let pid_path = hoosh_dir.join("daemon.pid");
            let pid = std::process::id();
            let _ = std::fs::write(&pid_path, pid.to_string());
        }

        let addr = self.config.bind_address;
        let router = self.router();

        let shutting_down = Arc::clone(&self.shutting_down);
        let active_tasks = Arc::clone(&self.active_tasks);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .with_context(|| format!("Failed to bind to {}", addr))?;

        eprintln!("hoosh daemon listening on {}", addr);

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

                eprintln!("hoosh daemon shutting down...");
                shutting_down.store(true, Ordering::Relaxed);

                let mut tasks = active_tasks.write().await;
                for (_, (handle, _)) in tasks.drain() {
                    let _ = handle.await;
                }

                if let Some(home) = dirs::home_dir() {
                    let _ = std::fs::remove_file(home.join(".hoosh").join("daemon.pid"));
                }
            })
            .await
            .context("Server error")?;

        Ok(())
    }
}

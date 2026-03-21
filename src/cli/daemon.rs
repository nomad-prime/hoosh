use anyhow::{Context, Result};
use std::sync::Arc;

use super::DaemonAction;
use crate::agent_definition::AgentDefinitionManager;
use crate::config::AppConfig;
use crate::console::console;
use crate::daemon::api::DaemonServer;
use crate::daemon::config::DaemonConfig;
use crate::daemon::executor::TaskExecutor;
use crate::daemon::store::TaskStore;

pub async fn handle_daemon(action: DaemonAction, config: AppConfig) -> Result<()> {
    let daemon_config = config.daemon.clone().unwrap_or_default();

    match action {
        DaemonAction::Start { port } => {
            let mut cfg = daemon_config;
            if let Some(p) = port {
                cfg.bind_address = format!("127.0.0.1:{}", p).parse().context("Invalid port")?;
            }
            daemon_start(cfg, &config).await
        }
        DaemonAction::Stop { force } => daemon_stop(force, &daemon_config).await,
        DaemonAction::Status => daemon_status(&daemon_config).await,
        DaemonAction::Submit {
            repo,
            branch,
            instructions,
            pr_title,
            labels,
            token_budget,
        } => {
            daemon_submit(
                repo,
                branch,
                instructions,
                pr_title,
                labels,
                token_budget,
                &daemon_config,
            )
            .await
        }
    }
}

async fn daemon_start(daemon_config: DaemonConfig, app_config: &AppConfig) -> Result<()> {
    use crate::backends::backend_factory::create_backend;

    for warning in daemon_config.github.startup_warnings() {
        console().warning(warning);
    }

    let backend = create_backend(&app_config.default_backend, app_config)
        .context("Failed to create LLM backend")?;
    backend.initialize().await?;
    let backend = Arc::from(backend);

    let store = Arc::new(TaskStore::new().context("Failed to create task store")?);

    let agent_manager =
        AgentDefinitionManager::new().context("Failed to load agent definitions")?;
    let agent = agent_manager
        .get_agent(&daemon_config.daemon_agent)
        .with_context(|| format!("Daemon agent '{}' not found", daemon_config.daemon_agent))?;

    let config = Arc::new(daemon_config);
    let executor = Arc::new(TaskExecutor::new(
        Arc::clone(&store),
        Arc::clone(&config),
        backend,
        agent.content,
        agent.core_instructions,
    ));

    let server = DaemonServer::new(config, store, executor);
    server.start().await
}

async fn daemon_stop(force: bool, _config: &DaemonConfig) -> Result<()> {
    let pid_path = AppConfig::hoosh_data_dir()
        .context("Could not determine data directory")?
        .join("daemon.pid");

    if !pid_path.exists() {
        console().plain("daemon is not running (no PID file)");
        return Ok(());
    }

    let pid_str = std::fs::read_to_string(&pid_path).context("Failed to read PID file")?;
    let pid: u32 = pid_str.trim().parse().context("Invalid PID in file")?;

    #[cfg(unix)]
    {
        use nix::sys::signal::{Signal, kill};
        use nix::unistd::Pid;
        use std::time::Duration;

        let nix_pid = Pid::from_raw(pid as i32);

        let signal = if force {
            Signal::SIGKILL
        } else {
            Signal::SIGTERM
        };

        kill(nix_pid, signal).with_context(|| format!("Failed to send signal to PID {}", pid))?;

        if !force {
            let timeout = Duration::from_secs(10);
            let start = std::time::Instant::now();
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if kill(nix_pid, None).is_err() {
                    break;
                }
                if start.elapsed() > timeout {
                    console().warning("Timed out waiting for daemon to stop. Use --force to kill.");
                    return Ok(());
                }
            }
        }

        let _ = std::fs::remove_file(&pid_path);
        console().success("daemon stopped");
    }

    #[cfg(not(unix))]
    {
        console().warning("daemon stop is only supported on Unix systems");
    }

    Ok(())
}

async fn daemon_status(config: &DaemonConfig) -> Result<()> {
    let pid_path = AppConfig::hoosh_data_dir()
        .context("Could not determine data directory")?
        .join("daemon.pid");

    if !pid_path.exists() {
        println!("daemon is not running");
        return Ok(());
    }

    let pid_str = std::fs::read_to_string(&pid_path).context("Failed to read PID file")?;
    let pid: u32 = pid_str.trim().parse().context("Invalid PID in file")?;

    #[cfg(unix)]
    {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;

        let nix_pid = Pid::from_raw(pid as i32);
        if kill(nix_pid, None).is_err() {
            println!("daemon is not running (stale PID file)");
            let _ = std::fs::remove_file(&pid_path);
            return Ok(());
        }
    }

    let port = config.bind_address.port();
    let health_url = format!("http://127.0.0.1:{}/health", port);

    match reqwest::get(&health_url).await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(body) = resp.text().await {
                println!("daemon is running (PID: {})", pid);
                println!("{}", body);
            } else {
                println!("daemon is running (PID: {})", pid);
            }
        }
        _ => {
            println!(
                "daemon process is running (PID: {}) but HTTP API is not responding",
                pid
            );
        }
    }

    Ok(())
}

async fn daemon_submit(
    repo: String,
    branch: String,
    instructions: String,
    pr_title: Option<String>,
    labels: Vec<String>,
    token_budget: Option<usize>,
    config: &DaemonConfig,
) -> Result<()> {
    let port = config.bind_address.port();
    let url = format!("http://127.0.0.1:{}/tasks", port);

    let body = serde_json::json!({
        "repo_url": repo,
        "base_branch": branch,
        "instructions": instructions,
        "pr_title": pr_title,
        "pr_labels": labels,
        "token_budget": token_budget,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("Failed to connect to daemon. Is it running?")?;

    if resp.status().is_success() {
        let value: serde_json::Value = resp.json().await.context("Failed to parse response")?;
        if let Some(task_id) = value.get("task_id").and_then(|v| v.as_str()) {
            println!("{}", task_id);
        }
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        console().error(&format!("Error {}: {}", status, body));
    }

    Ok(())
}

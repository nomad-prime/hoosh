use anyhow::Result;
use async_trait::async_trait;
use hoosh::backends::{LlmBackend, LlmError, LlmResponse};
use hoosh::daemon::api::DaemonServer;
use hoosh::daemon::config::DaemonConfig;
use hoosh::daemon::executor::TaskExecutor;
use hoosh::daemon::pr_provider::{CreatePrParams, PrProvider, PrResult};
use hoosh::daemon::store::TaskStore;
use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::TempDir;

struct MockBackend;

#[async_trait]
impl LlmBackend for MockBackend {
    async fn send_message(&self, _message: &str) -> Result<String> {
        Ok("done".to_string())
    }

    async fn send_message_with_tools(
        &self,
        _conversation: &hoosh::agent::Conversation,
        _tools: &hoosh::tools::ToolRegistry,
    ) -> Result<LlmResponse, LlmError> {
        Ok(
            LlmResponse::content_only("Task complete, no changes needed.".to_string())
                .with_tokens(10, 10),
        )
    }

    fn backend_name(&self) -> &str {
        "mock"
    }

    fn model_name(&self) -> &str {
        "mock-model"
    }
}

struct MockPrProvider;

#[async_trait]
impl PrProvider for MockPrProvider {
    async fn create_pull_request(&self, _params: CreatePrParams) -> Result<PrResult> {
        Ok(PrResult {
            pr_url: "https://github.com/owner/repo/pull/42".to_string(),
            pr_number: 42,
        })
    }

    fn provider_name(&self) -> &'static str {
        "mock"
    }
}

fn init_bare_with_commit(path: &std::path::Path) {
    let repo = git2::Repository::init_bare(path).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    let tree_oid = {
        let builder = repo.treebuilder(None).unwrap();
        builder.write().unwrap()
    };
    let tree = repo.find_tree(tree_oid).unwrap();
    repo.commit(
        Some("refs/heads/main"),
        &sig,
        &sig,
        "Initial commit",
        &tree,
        &[],
    )
    .unwrap();
}

struct TestServer {
    addr: SocketAddr,
    _store_dir: TempDir,
    _sandbox_dir: TempDir,
    _remote_dir: TempDir,
    _store: Arc<TaskStore>,
}

async fn start_test_server() -> TestServer {
    let remote_dir = TempDir::new().unwrap();
    init_bare_with_commit(remote_dir.path());
    let repo_url = format!("file://{}", remote_dir.path().display());

    let store_dir = TempDir::new().unwrap();
    let store = Arc::new(TaskStore::new_with_dir(store_dir.path().join("tasks")).unwrap());

    let sandbox_dir = TempDir::new().unwrap();

    let config = DaemonConfig {
        sandbox_base_dir: sandbox_dir.path().to_path_buf(),
        bind_address: "127.0.0.1:0".parse().unwrap(),
        ..Default::default()
    };
    let config = Arc::new(config);

    let executor = Arc::new(TaskExecutor::new(
        Arc::clone(&store),
        Arc::clone(&config),
        Arc::new(MockPrProvider),
        Arc::new(MockBackend),
    ));

    let server = DaemonServer::new(Arc::clone(&config), Arc::clone(&store), executor);
    let router = server.router();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Store repo_url for use in tests (it's passed via the request body)
    let _ = repo_url;

    TestServer {
        addr,
        _store_dir: store_dir,
        _sandbox_dir: sandbox_dir,
        _remote_dir: remote_dir,
        _store: store,
    }
}

fn repo_url(srv: &TestServer) -> String {
    format!("file://{}", srv._remote_dir.path().display())
}

#[tokio::test]
async fn submit_task_no_changes_completes_without_pr() {
    let srv = start_test_server().await;
    let client = reqwest::Client::new();
    let url = format!("http://{}/tasks", srv.addr);

    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "repo_url": repo_url(&srv),
            "base_branch": "main",
            "instructions": "Do nothing",
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 202);
    let body: serde_json::Value = resp.json().await.unwrap();
    let task_id = body["task_id"].as_str().unwrap().to_string();

    // Poll until terminal
    let poll_url = format!("http://{}/tasks/{}", srv.addr, task_id);
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let task: serde_json::Value = client
            .get(&poll_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let status = task["status"].as_str().unwrap();
        if status == "Completed" || status == "Failed" || status == "Cancelled" {
            assert_eq!(status, "Completed");
            assert!(task["pr_url"].is_null());
            return;
        }
    }
    panic!("Task did not complete within timeout");
}

#[tokio::test]
async fn submit_with_missing_field_returns_400() {
    let srv = start_test_server().await;
    let client = reqwest::Client::new();
    let url = format!("http://{}/tasks", srv.addr);

    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "repo_url": "",
            "base_branch": "main",
            "instructions": "Do something",
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn get_unknown_task_returns_404() {
    let srv = start_test_server().await;
    let client = reqwest::Client::new();
    let url = format!("http://{}/tasks/nonexistent-task-id", srv.addr);

    let resp = client.get(&url).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn cancel_completed_task_returns_409() {
    let srv = start_test_server().await;
    let client = reqwest::Client::new();

    // Submit and wait for completion
    let submit_url = format!("http://{}/tasks", srv.addr);
    let resp = client
        .post(&submit_url)
        .json(&serde_json::json!({
            "repo_url": repo_url(&srv),
            "base_branch": "main",
            "instructions": "Do nothing quickly",
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    let task_id = body["task_id"].as_str().unwrap().to_string();

    let poll_url = format!("http://{}/tasks/{}", srv.addr, task_id);
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let task: serde_json::Value = client
            .get(&poll_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if task["status"].as_str().unwrap() != "Pending"
            && task["status"].as_str().unwrap() != "Running"
        {
            break;
        }
    }

    let cancel_url = format!("http://{}/tasks/{}", srv.addr, task_id);
    let resp = client.delete(&cancel_url).send().await.unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn list_tasks_returns_all_tasks() {
    let srv = start_test_server().await;
    let client = reqwest::Client::new();
    let submit_url = format!("http://{}/tasks", srv.addr);

    client
        .post(&submit_url)
        .json(&serde_json::json!({
            "repo_url": repo_url(&srv),
            "base_branch": "main",
            "instructions": "Task A",
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let list_url = format!("http://{}/tasks", srv.addr);
    let tasks: Vec<serde_json::Value> = client
        .get(&list_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!tasks.is_empty());
}

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let srv = start_test_server().await;
    let client = reqwest::Client::new();
    let url = format!("http://{}/health", srv.addr);

    let resp = client.get(&url).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

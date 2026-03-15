use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use serde::Deserialize;

use super::{CreatePrParams, PrProvider, PrResult};

pub struct GitHubPrProvider {
    pat: String,
    client: reqwest::Client,
    api_base: String,
}

impl GitHubPrProvider {
    pub fn new(pat: String) -> Self {
        Self {
            pat,
            client: reqwest::Client::new(),
            api_base: "https://api.github.com".to_string(),
        }
    }

    pub fn with_api_base(mut self, api_base: String) -> Self {
        self.api_base = api_base;
        self
    }
}

pub fn parse_github_remote(url: &str) -> Result<(String, String)> {
    if url.starts_with("git@github.com:") {
        let rest = url.trim_start_matches("git@github.com:");
        let rest = rest.trim_end_matches(".git");
        let parts: Vec<&str> = rest.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
        bail!("Invalid SSH GitHub URL format: {}", url);
    }

    if url.starts_with("https://github.com/") || url.starts_with("http://github.com/") {
        let rest = url
            .trim_start_matches("https://github.com/")
            .trim_start_matches("http://github.com/");
        let rest = rest.trim_end_matches(".git");
        let parts: Vec<&str> = rest.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
        bail!("Invalid HTTPS GitHub URL format: {}", url);
    }

    bail!(
        "Unrecognized GitHub remote URL format: '{}'. Expected SSH (git@github.com:owner/repo.git) or HTTPS (https://github.com/owner/repo)",
        url
    )
}

#[derive(Deserialize)]
struct CreatePrResponse {
    html_url: String,
    number: u64,
}

#[derive(Deserialize)]
struct GitHubErrorResponse {
    message: String,
}

#[async_trait]
impl PrProvider for GitHubPrProvider {
    async fn create_pull_request(&self, params: CreatePrParams) -> Result<PrResult> {
        let (owner, repo) = parse_github_remote(&params.repo_url)
            .context("Failed to parse GitHub repository URL")?;

        let url = format!("{}/repos/{}/{}/pulls", self.api_base, owner, repo);

        let body = serde_json::json!({
            "title": params.title,
            "body": params.body,
            "head": params.head_branch,
            "base": params.base_branch,
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.pat))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "hoosh-daemon/1.0")
            .json(&body)
            .send()
            .await
            .context("Failed to send PR creation request to GitHub API")?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            if let Ok(err) = serde_json::from_str::<GitHubErrorResponse>(&text) {
                bail!("GitHub API error ({}): {}", status, err.message);
            }
            bail!("GitHub API returned error status {}: {}", status, text);
        }

        let pr: CreatePrResponse = response
            .json()
            .await
            .context("Failed to parse GitHub PR creation response")?;

        Ok(PrResult {
            pr_url: pr.html_url,
            pr_number: pr.number,
        })
    }

    fn provider_name(&self) -> &'static str {
        "github"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn make_provider(server: &MockServer) -> GitHubPrProvider {
        GitHubPrProvider::new("test-pat".to_string()).with_api_base(server.base_url())
    }

    fn make_params() -> CreatePrParams {
        CreatePrParams {
            repo_url: "https://github.com/owner/repo".to_string(),
            head_branch: "feature/branch".to_string(),
            base_branch: "main".to_string(),
            title: "Test PR".to_string(),
            body: "Test body".to_string(),
            labels: vec![],
        }
    }

    #[tokio::test]
    async fn creates_pr_returns_url() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/repos/owner/repo/pulls")
                .header("Authorization", "Bearer test-pat");
            then.status(201)
                .header("Content-Type", "application/json")
                .body(r#"{"html_url": "https://github.com/owner/repo/pull/42", "number": 42}"#);
        });

        let provider = make_provider(&server);
        let result = provider.create_pull_request(make_params()).await.unwrap();

        assert_eq!(result.pr_url, "https://github.com/owner/repo/pull/42");
        assert_eq!(result.pr_number, 42);
        mock.assert();
    }

    #[tokio::test]
    async fn handles_api_error_with_message() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/repos/owner/repo/pulls");
            then.status(422)
                .header("Content-Type", "application/json")
                .body(r#"{"message": "Validation Failed", "errors": []}"#);
        });

        let provider = make_provider(&server);
        let result = provider.create_pull_request(make_params()).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Validation Failed"),
            "Error should contain API message: {}",
            err
        );
    }

    #[test]
    fn parses_ssh_url_extracts_owner_and_repo() {
        let (owner, repo) = parse_github_remote("git@github.com:myorg/myrepo.git").unwrap();
        assert_eq!(owner, "myorg");
        assert_eq!(repo, "myrepo");
    }

    #[test]
    fn parses_https_url_extracts_owner_and_repo() {
        let (owner, repo) = parse_github_remote("https://github.com/myorg/myrepo.git").unwrap();
        assert_eq!(owner, "myorg");
        assert_eq!(repo, "myrepo");
    }

    #[test]
    fn parses_https_url_without_git_suffix() {
        let (owner, repo) = parse_github_remote("https://github.com/myorg/myrepo").unwrap();
        assert_eq!(owner, "myorg");
        assert_eq!(repo, "myrepo");
    }

    #[test]
    fn invalid_url_returns_clear_error() {
        let result = parse_github_remote("https://notgithub.com/owner/repo");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Unrecognized"),
            "Error should explain the problem: {}",
            err
        );
    }
}

pub mod github;

use anyhow::Result;
use async_trait::async_trait;

pub struct CreatePrParams {
    pub repo_url: String,
    pub head_branch: String,
    pub base_branch: String,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
}

#[derive(Debug)]
pub struct PrResult {
    pub pr_url: String,
    pub pr_number: u64,
}

#[deprecated(note = "Use gh CLI via agent for PR creation")]
#[async_trait]
pub trait PrProvider: Send + Sync {
    async fn create_pull_request(&self, params: CreatePrParams) -> Result<PrResult>;
    fn provider_name(&self) -> &'static str;
}

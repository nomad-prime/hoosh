use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMetadata {
    pub description: String,

    #[serde(default)]
    pub handoffs: Vec<Handoff>,

    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handoff {
    pub label: String,
    pub agent: String,
    pub prompt: String,

    #[serde(default)]
    pub send: bool,
}

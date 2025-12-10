use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalateTool {
    pub name: String,
    pub description: String,
}

impl EscalateTool {
    pub fn new() -> Self {
        Self {
            name: "escalate".to_string(),
            description: "Escalate to next model tier for increased capability".to_string(),
        }
    }
}

impl Default for EscalateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escalate_tool_creation() {
        let tool = EscalateTool::new();
        assert_eq!(tool.name, "escalate");
        assert!(!tool.description.is_empty());
    }
}

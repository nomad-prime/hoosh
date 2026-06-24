use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

mod execution_budget;
pub use execution_budget::{BudgetInfo, ExecutionBudget};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentType {
    Plan,
    Explore,
    Review,
}

impl AgentType {
    pub const ALL: [AgentType; 3] = [AgentType::Plan, AgentType::Explore, AgentType::Review];

    pub fn as_str(&self) -> &'static str {
        match self {
            AgentType::Plan => "plan",
            AgentType::Explore => "explore",
            AgentType::Review => "review",
        }
    }

    pub fn names() -> Vec<&'static str> {
        Self::ALL.iter().map(AgentType::as_str).collect()
    }

    pub fn from_name(s: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|t| t.as_str() == s.to_lowercase())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Unknown agent type: {}. Valid types are: {}",
                    s,
                    Self::names().join(", ")
                )
            })
    }

    pub fn max_steps(&self) -> usize {
        match self {
            AgentType::Plan => 100,
            AgentType::Explore => 75,
            AgentType::Review => 75,
        }
    }

    pub fn default_timeout_seconds(&self) -> u64 {
        match self {
            AgentType::Plan => 600,
            AgentType::Explore => 300,
            AgentType::Review => 600,
        }
    }

    pub fn default_thinking_budget(&self) -> Option<u32> {
        match self {
            AgentType::Plan => Some(5000),
            AgentType::Review => Some(3000),
            AgentType::Explore => None,
        }
    }

    pub fn when_to_use(&self) -> &'static str {
        match self {
            AgentType::Plan => {
                "Architect agent for designing implementation plans. Use for complex feature planning, architecture decisions, or multi-file refactoring strategies. Returns a step-by-step plan; does not write code. (max 100 steps, 600s timeout)"
            }
            AgentType::Explore => {
                "Fast read-only search agent for locating code. Use it to find files by pattern, grep for symbols or keywords, or answer 'where is X defined / which files reference Y'. Specify search breadth in the prompt. (max 75 steps, 300s timeout)"
            }
            AgentType::Review => {
                "Read-only code review agent for correctness bugs, security issues, and convention checks. Use for PR reviews, audits, and cross-file consistency checks. (max 75 steps, 600s timeout)"
            }
        }
    }

    pub fn system_message(&self, task_prompt: &str, budget: Option<&ExecutionBudget>) -> String {
        let base = match self {
            AgentType::Plan => include_str!("../prompts/hoosh_planner.txt"),
            AgentType::Explore => include_str!("../prompts/hoosh_explore.txt"),
            AgentType::Review => include_str!("../prompts/hoosh_reviewer.txt"),
        };

        let mut message = format!("{}\n\nTask: {}", base, task_prompt);

        if let Some(budget) = budget {
            let budget_guidance = format!(
                "\n\n[EXECUTION BUDGET]\nYou have a time limit of {} seconds and a maximum of {} steps to complete this task. \
                You will receive budget updates as you progress. When approaching your limits, prioritize completing your work efficiently.",
                budget.max_duration.as_secs(),
                self.max_steps()
            );
            message.push_str(&budget_guidance);
        }

        message
    }
}

#[derive(Debug, Clone)]
pub struct TaskDefinition {
    pub agent_type: AgentType,
    pub prompt: String,
    pub description: String,
    pub timeout_seconds: Option<u64>,
    pub model: Option<String>,
    pub budget: Option<ExecutionBudget>,
    pub thinking_budget: Option<u32>,
}

impl TaskDefinition {
    pub fn new(agent_type: AgentType, prompt: String, description: String) -> Self {
        let timeout_seconds = Some(agent_type.default_timeout_seconds());
        let thinking_budget = agent_type.default_thinking_budget();
        Self {
            agent_type,
            prompt,
            description,
            timeout_seconds,
            model: None,
            budget: None,
            thinking_budget,
        }
    }

    pub fn with_model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = Some(timeout_seconds);
        self
    }

    pub fn with_thinking_budget(mut self, thinking_budget: Option<u32>) -> Self {
        self.thinking_budget = thinking_budget;
        self
    }

    pub fn initialize_budget(mut self) -> Self {
        if let Some(timeout) = self.timeout_seconds {
            self.budget = Some(ExecutionBudget::new(
                Duration::from_secs(timeout),
                self.agent_type.max_steps(),
            ));
        }
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskEvent {
    pub event_type: String,
    pub message: String,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub struct TaskResult {
    pub success: bool,
    pub output: String,
    pub events: Vec<TaskEvent>,
    pub token_usage: Option<TokenUsage>,
    pub budget_info: Option<BudgetInfo>,
}

impl TaskResult {
    pub fn success(output: String) -> Self {
        Self {
            success: true,
            output,
            events: Vec::new(),
            token_usage: None,
            budget_info: None,
        }
    }

    pub fn failure(error: String) -> Self {
        Self {
            success: false,
            output: error,
            events: Vec::new(),
            token_usage: None,
            budget_info: None,
        }
    }

    pub fn with_events(mut self, events: Vec<TaskEvent>) -> Self {
        self.events = events;
        self
    }

    pub fn with_token_usage(mut self, token_usage: TokenUsage) -> Self {
        self.token_usage = Some(token_usage);
        self
    }

    pub fn with_budget_info(mut self, budget_info: BudgetInfo) -> Self {
        self.budget_info = Some(budget_info);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
}

pub mod task_manager;

pub use task_manager::TaskManager;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_type_from_str() {
        assert!(matches!(AgentType::from_name("plan"), Ok(AgentType::Plan)));
        assert!(matches!(
            AgentType::from_name("explore"),
            Ok(AgentType::Explore)
        ));
        assert!(AgentType::from_name("invalid").is_err());
    }

    #[test]
    fn test_agent_type_max_steps() {
        assert_eq!(AgentType::Plan.max_steps(), 100);
        assert_eq!(AgentType::Explore.max_steps(), 75);
    }

    #[test]
    fn test_task_definition_builder() {
        let task = TaskDefinition::new(
            AgentType::Plan,
            "analyze code".to_string(),
            "code analysis".to_string(),
        );
        assert_eq!(task.timeout_seconds, Some(600));
        assert_eq!(task.model, None);

        let task_with_model = task.clone().with_model("gpt-4".to_string());
        assert_eq!(task_with_model.model, Some("gpt-4".to_string()));
        assert_eq!(task_with_model.timeout_seconds, Some(600));
    }

    #[test]
    fn test_task_result_success() {
        let result = TaskResult::success("output text".to_string());
        assert!(result.success);
        assert_eq!(result.output, "output text");
        assert!(result.events.is_empty());
        assert!(result.token_usage.is_none());
    }

    #[test]
    fn test_task_result_failure() {
        let result = TaskResult::failure("error message".to_string());
        assert!(!result.success);
        assert_eq!(result.output, "error message");
        assert!(result.events.is_empty());
    }

    #[test]
    fn test_task_result_with_events() {
        let event = TaskEvent {
            event_type: "test".to_string(),
            message: "test message".to_string(),
            timestamp: std::time::SystemTime::now(),
        };
        let result = TaskResult::success("output".to_string()).with_events(vec![event]);
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].event_type, "test");
    }

    #[test]
    fn test_task_result_with_token_usage() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
        };
        let result = TaskResult::success("output".to_string()).with_token_usage(usage);
        assert!(result.token_usage.is_some());
        assert_eq!(result.token_usage.as_ref().unwrap().input_tokens, 100);
        assert_eq!(result.token_usage.as_ref().unwrap().output_tokens, 50);
    }

    #[test]
    fn test_agent_type_review() {
        assert!(matches!(
            AgentType::from_name("review"),
            Ok(AgentType::Review)
        ));
        assert_eq!(AgentType::Review.max_steps(), 75);
    }

    #[test]
    fn test_agent_type_default_thinking_budget() {
        assert_eq!(AgentType::Plan.default_thinking_budget(), Some(5000));
        assert_eq!(AgentType::Review.default_thinking_budget(), Some(3000));
        assert_eq!(AgentType::Explore.default_thinking_budget(), None);
    }

    #[test]
    fn test_task_definition_inherits_thinking_budget() {
        let plan_task = TaskDefinition::new(AgentType::Plan, "p".to_string(), "d".to_string());
        assert_eq!(plan_task.thinking_budget, Some(5000));

        let explore_task =
            TaskDefinition::new(AgentType::Explore, "p".to_string(), "d".to_string());
        assert_eq!(explore_task.thinking_budget, None);

        let overridden = plan_task.with_thinking_budget(Some(1000));
        assert_eq!(overridden.thinking_budget, Some(1000));
    }

    #[test]
    fn test_review_system_message() {
        let msg = AgentType::Review.system_message("Review auth code", None);
        assert!(msg.contains("code review"));
        assert!(msg.contains("Review auth code"));
    }
}

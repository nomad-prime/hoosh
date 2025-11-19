mod context_manager;
mod sliding_window_strategy;
mod summarizer;
mod token_accountant;
mod tool_output_truncation_strategy;

use serde::{Deserialize, Serialize};

/// Result of applying a context management strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategyResult {
    /// Strategy was applied and modified the conversation
    Applied,

    /// Strategy was applied but made no changes (already within target)
    NoChange,

    /// Strategy reached the token target, stop further processing
    TargetReached,
}

pub use context_manager::{
    ContextManagementStrategy, ContextManager, ContextManagerConfig, SlidingWindowConfig,
    ToolOutputTruncationConfig,
};
pub use sliding_window_strategy::SlidingWindowStrategy;
pub use summarizer::MessageSummarizer;
pub use token_accountant::{TokenAccountant, TokenAccountantStats, TokenUsageRecord};
pub use tool_output_truncation_strategy::ToolOutputTruncationStrategy;

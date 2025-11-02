pub mod agents;
pub mod backends;
pub mod cli;
pub mod commands;
pub mod completion;
pub mod config;
pub mod console;
pub mod conversations;
pub mod history;
pub mod parser;
pub mod permissions;
pub mod security;
pub mod tool_executor;
pub mod tools;
pub mod tui;

pub use agents::{Agent, AgentManager};
#[cfg(feature = "anthropic")]
pub use backends::{AnthropicBackend, AnthropicConfig};
pub use backends::{LlmBackend, LlmResponse};
#[cfg(feature = "openai-compatible")]
pub use backends::{OpenAICompatibleBackend, OpenAICompatibleConfig};
#[cfg(feature = "together-ai")]
pub use backends::{TogetherAiBackend, TogetherAiConfig};
pub use commands::{
    register_default_commands, Command, CommandContext, CommandRegistry, CommandResult,
};
pub use config::{AgentConfig, AppConfig, BackendConfig};
pub use console::{console, init_console, Console, VerbosityLevel};
pub use conversations::{
    AgentEvent, Conversation, ConversationHandler, ConversationMessage, ToolCall, ToolCallResponse,
    ToolExecutionContext, ToolFunction,
};
pub use parser::MessageParser;
pub use permissions::PermissionManager;
pub use permissions::{ToolPermissionBuilder, ToolPermissionDescriptor};
pub use security::PathValidator;
pub use tool_executor::ToolExecutor;
pub use tools::{Tool, ToolRegistry};

pub mod agent;
pub mod agent_definition;
pub mod backends;
pub mod cli;
pub mod commands;
pub mod completion;
pub mod config;
pub mod console;
pub mod context_management;
pub mod history;
pub mod parser;
pub mod permissions;
pub mod security;
pub mod session;
pub mod storage;
pub mod task_management;
pub mod tool_executor;
pub mod tools;
pub mod tui;

pub use agent::{
    Agent, AgentEvent, Conversation, ConversationMessage, ToolCall, ToolCallResponse,
    ToolExecutionContext, ToolFunction,
};
pub use agent_definition::{AgentDefinition, AgentDefinitionManager};
#[cfg(feature = "anthropic")]
pub use backends::{AnthropicBackend, AnthropicConfig};
pub use backends::{LlmBackend, LlmResponse};
#[cfg(feature = "openai-compatible")]
pub use backends::{OpenAICompatibleBackend, OpenAICompatibleConfig};
#[cfg(feature = "together-ai")]
pub use backends::{TogetherAiBackend, TogetherAiConfig};
pub use commands::{
    Command, CommandContext, CommandRegistry, CommandResult, register_default_commands,
};
pub use config::{AgentConfig, AppConfig, BackendConfig};
pub use console::{Console, VerbosityLevel, console, init_console};
pub use parser::MessageParser;
pub use permissions::PermissionManager;
pub use permissions::{ToolPermissionBuilder, ToolPermissionDescriptor};
pub use security::PathValidator;
pub use storage::{ConversationMetadata, ConversationStorage};
pub use tool_executor::ToolExecutor;
pub use tools::{TaskToolProvider, Tool, ToolRegistry};

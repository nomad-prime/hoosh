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
pub mod session_files;
pub mod skill_management;
pub mod storage;
pub mod system_reminders;
pub mod tagged_mode;
pub mod task_management;
pub mod terminal_capabilities;
pub mod terminal_markdown;
pub mod terminal_mode;
pub mod terminal_spinner;
pub mod text_prompts;
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
pub use session_files::{SessionFile, cleanup_stale_sessions, get_terminal_pid};
pub use skill_management::{Skill, SkillManager};
pub use storage::{ConversationMetadata, ConversationStorage};
pub use terminal_capabilities::TerminalCapabilities;
pub use terminal_mode::{TerminalMode, select_terminal_mode};
pub use tool_executor::ToolExecutor;
pub use tools::{BuiltinToolProvider, ReadOnlyToolProvider, TaskToolProvider, Tool, ToolRegistry};

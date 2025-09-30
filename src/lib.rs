pub mod backends;
pub mod cli;
pub mod config;
pub mod console;
pub mod conversation;
pub mod input;
pub mod parser;
pub mod permissions;
pub mod system_prompts;
pub mod tool_executor;
pub mod tools;

pub use backends::{LlmBackend, LlmResponse, StreamResponse};
#[cfg(feature = "together-ai")]
pub use backends::{TogetherAiBackend, TogetherAiConfig};
pub use config::{AppConfig, BackendConfig, PromptConfig};
pub use console::{Console, VerbosityLevel, console, init_console};
pub use conversation::{Conversation, ConversationMessage, ToolCall, ToolExecutionContext, ToolFunction, ToolResult};
pub use input::InputHandler;
pub use parser::MessageParser;
pub use permissions::PermissionManager;
pub use tool_executor::ToolExecutor;
pub use system_prompts::{SystemPrompt, SystemPromptManager};
pub use tools::{Tool, ToolRegistry};

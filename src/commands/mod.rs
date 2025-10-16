mod agents_command;
mod clear_command;
mod exit_command;
mod help_command;
mod register;
mod registry;
mod status_command;
mod tools_command;

pub use register::register_default_commands;
pub use registry::{Command, CommandContext, CommandRegistry, CommandResult};

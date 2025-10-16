use anyhow::Result;
use std::sync::Arc;

use super::agents_command::AgentsCommand;
use super::clear_command::ClearCommand;
use super::exit_command::ExitCommand;
use super::help_command::HelpCommand;
use super::registry::CommandRegistry;
use super::status_command::StatusCommand;
use super::tools_command::ToolsCommand;

pub fn register_default_commands(registry: &mut CommandRegistry) -> Result<()> {
    registry.register(Arc::new(HelpCommand))?;
    registry.register(Arc::new(ClearCommand))?;
    registry.register(Arc::new(StatusCommand))?;
    registry.register(Arc::new(ToolsCommand))?;
    registry.register(Arc::new(AgentsCommand))?;
    registry.register(Arc::new(ExitCommand))?;
    Ok(())
}

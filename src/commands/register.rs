use anyhow::Result;
use std::sync::Arc;

use super::agents_command::AgentsCommand;
use super::clear_command::ClearCommand;
use super::custom::CustomCommandManager;
use super::exit_command::ExitCommand;
use super::help_command::HelpCommand;
use super::permissions_command::PermissionsCommand;
use super::registry::CommandRegistry;
use super::rename_command::RenameCommand;
use super::status_command::StatusCommand;
use super::tools_command::ToolsCommand;
use super::untrust_command::UntrustCommand;

pub fn register_default_commands(registry: &mut CommandRegistry) -> Result<()> {
    registry.register(Arc::new(HelpCommand))?;
    registry.register(Arc::new(ClearCommand))?;
    registry.register(Arc::new(StatusCommand))?;
    registry.register(Arc::new(ToolsCommand))?;
    registry.register(Arc::new(AgentsCommand))?;
    registry.register(Arc::new(ExitCommand))?;
    registry.register(Arc::new(UntrustCommand))?;
    registry.register(Arc::new(PermissionsCommand))?;
    registry.register(Arc::new(RenameCommand))?;
    Ok(())
}

pub fn register_custom_commands(registry: &mut CommandRegistry) -> Result<usize> {
    let mut manager = CustomCommandManager::new()?;
    manager.load_commands()?;
    let count = manager.register_commands(registry)?;
    Ok(count)
}

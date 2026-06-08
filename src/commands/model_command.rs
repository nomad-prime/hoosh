use anyhow::{Result, anyhow};
use async_trait::async_trait;

use super::registry::{Command, CommandContext, CommandResult};
use crate::agent::AgentEvent;

pub struct ModelCommand;

#[async_trait]
impl Command for ModelCommand {
    fn name(&self) -> &str {
        "model"
    }

    fn description(&self) -> &str {
        "Show or switch the model used by the active backend"
    }

    fn usage(&self) -> &str {
        "/model [name] [--save]\n\n\
         With no argument: prints the current model.\n\
         With a name: switches the active backend to that model for the rest of the session.\n\
         With --save: also writes the choice to the config file.\n\n\
         Examples:\n  \
           /model\n  \
           /model claude-opus-4-7\n  \
           /model llama3.1 --save"
    }

    async fn execute(
        &self,
        args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        let backend = context
            .backend
            .as_ref()
            .ok_or_else(|| anyhow!("No active backend"))?;

        let (positional, save) = parse_args(&args);

        if positional.is_empty() {
            let mut msg = format!(
                "Current backend: {}\nCurrent model: {}\n",
                backend.backend_name(),
                backend.model_name()
            );
            if let Some(p) = backend.pricing() {
                msg.push_str(&format!(
                    "Pricing: ${:.2}/M input, ${:.2}/M output\n",
                    p.input_per_million, p.output_per_million
                ));
            }
            msg.push_str("\nUsage: /model <name> [--save]");
            return Ok(CommandResult::Success(msg));
        }

        let target = positional[0].to_string();
        let event_tx = context
            .event_tx
            .as_ref()
            .ok_or_else(|| anyhow!("Event channel not available"))?;

        event_tx
            .send(AgentEvent::SwitchBackend {
                backend: None,
                model: Some(target.clone()),
                save,
            })
            .map_err(|e| anyhow!("Failed to dispatch model switch: {e}"))?;

        let mut msg = format!(
            "Switching model to '{target}' on backend '{}'",
            backend.backend_name()
        );
        if save {
            msg.push_str(" (and saving to config)");
        }
        msg.push('…');
        Ok(CommandResult::Success(msg))
    }
}

fn parse_args(args: &[String]) -> (Vec<&str>, bool) {
    let mut positional = Vec::new();
    let mut save = false;
    for a in args {
        if a == "--save" {
            save = true;
        } else {
            positional.push(a.as_str());
        }
    }
    (positional, save)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_extracts_save_flag() {
        let args: Vec<String> = vec!["claude-opus-4-7".into(), "--save".into()];
        let (pos, save) = parse_args(&args);
        assert_eq!(pos, vec!["claude-opus-4-7"]);
        assert!(save);
    }

    #[test]
    fn parse_args_no_save() {
        let args: Vec<String> = vec!["gpt-4".into()];
        let (pos, save) = parse_args(&args);
        assert_eq!(pos, vec!["gpt-4"]);
        assert!(!save);
    }
}

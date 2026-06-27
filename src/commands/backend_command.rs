use anyhow::{Result, anyhow};
use async_trait::async_trait;

use super::registry::{Command, CommandContext, CommandResult};
use crate::agent::AgentEvent;

pub struct BackendCommand;

#[async_trait]
impl Command for BackendCommand {
    fn name(&self) -> &str {
        "backend"
    }

    fn description(&self) -> &str {
        "Show or switch the active LLM backend"
    }

    fn usage(&self) -> &str {
        "/backend [name] [--save]\n\n\
         With no argument: prints the current backend and lists available ones.\n\
         With a name: switches to that backend for the rest of the session.\n\
         With --save: also writes the choice to the config file.\n\n\
         Examples:\n  \
           /backend\n  \
           /backend anthropic\n  \
           /backend ollama --save"
    }

    async fn execute(
        &self,
        args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        let config = context
            .config
            .as_ref()
            .ok_or_else(|| anyhow!("Config not available"))?;

        let (positional, save) = parse_args(&args);

        if positional.is_empty() {
            return Ok(CommandResult::Success(render_status(
                config,
                context.backend.as_deref(),
            )));
        }

        let target = positional[0].to_string();
        if !config.backends.contains_key(&target) {
            let available: Vec<&str> = config.backends.keys().map(String::as_str).collect();
            return Ok(CommandResult::Success(format!(
                "Unknown backend '{}'. Available: {}",
                target,
                available.join(", ")
            )));
        }

        let event_tx = context
            .event_tx
            .as_ref()
            .ok_or_else(|| anyhow!("Event channel not available"))?;

        event_tx
            .send(AgentEvent::SwitchBackend {
                backend: Some(target.clone()),
                model: None,
                save,
            })
            .map_err(|e| anyhow!("Failed to dispatch backend switch: {e}"))?;

        let mut msg = format!("Switching backend to '{target}'");
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

fn render_status(
    config: &crate::config::AppConfig,
    current_backend: Option<&dyn crate::backends::LlmBackend>,
) -> String {
    let mut out = String::new();
    if let Some(b) = current_backend {
        out.push_str(&format!(
            "Current: {} (model: {})\n",
            b.backend_name(),
            b.model_name()
        ));
        if let Some(p) = b.pricing() {
            out.push_str(&format!(
                "Pricing: ${:.2}/M input, ${:.2}/M output\n",
                p.input_per_million, p.output_per_million
            ));
        }
    } else {
        out.push_str(&format!(
            "Current (from config): {}\n",
            config.default_backend
        ));
    }

    out.push_str("\nAvailable backends:\n");
    let mut names: Vec<&String> = config.backends.keys().collect();
    names.sort();
    for name in names {
        let model = config
            .backends
            .get(name)
            .and_then(|b| b.model.as_deref())
            .unwrap_or("(no default model)");
        let marker = if name == &config.default_backend {
            "*"
        } else {
            " "
        };
        out.push_str(&format!("  {marker} {name:<16} {model}\n"));
    }
    out.push_str("\nUsage: /backend <name> [--save]");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_extracts_save_flag() {
        let args: Vec<String> = vec!["anthropic".into(), "--save".into()];
        let (pos, save) = parse_args(&args);
        assert_eq!(pos, vec!["anthropic"]);
        assert!(save);
    }

    #[test]
    fn parse_args_accepts_save_before_name() {
        let args: Vec<String> = vec!["--save".into(), "ollama".into()];
        let (pos, save) = parse_args(&args);
        assert_eq!(pos, vec!["ollama"]);
        assert!(save);
    }

    #[test]
    fn parse_args_no_save() {
        let args: Vec<String> = vec!["openai".into()];
        let (pos, save) = parse_args(&args);
        assert_eq!(pos, vec!["openai"]);
        assert!(!save);
    }

    #[tokio::test]
    async fn unknown_backend_returns_helpful_message() {
        let mut config = crate::config::AppConfig::default();
        config.backends.insert(
            "anthropic".into(),
            crate::config::BackendConfig {
                api_key: None,
                model: None,
                base_url: None,
                chat_api: None,
                temperature: None,
                pricing_endpoint: None,
                thinking_budget: None,
                reasoning_effort: None,
                streaming: None,
            },
        );
        let mut ctx = CommandContext::new().with_config(config);
        let result = BackendCommand
            .execute(vec!["does-not-exist".into()], &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Success(msg) => {
                assert!(msg.contains("Unknown backend"));
                assert!(msg.contains("anthropic"));
            }
            _ => panic!("expected success message"),
        }
    }

    #[tokio::test]
    async fn no_args_returns_status_listing() {
        let mut config = crate::config::AppConfig {
            default_backend: "anthropic".into(),
            ..crate::config::AppConfig::default()
        };
        config.backends.insert(
            "anthropic".into(),
            crate::config::BackendConfig {
                api_key: None,
                model: None,
                base_url: None,
                chat_api: None,
                temperature: None,
                pricing_endpoint: None,
                thinking_budget: None,
                reasoning_effort: None,
                streaming: None,
            },
        );
        config.backends.insert(
            "ollama".into(),
            crate::config::BackendConfig {
                api_key: None,
                model: None,
                base_url: None,
                chat_api: None,
                temperature: None,
                pricing_endpoint: None,
                thinking_budget: None,
                reasoning_effort: None,
                streaming: None,
            },
        );
        let mut ctx = CommandContext::new().with_config(config);
        let result = BackendCommand.execute(vec![], &mut ctx).await.unwrap();
        match result {
            CommandResult::Success(msg) => {
                assert!(msg.contains("Available backends"));
                assert!(msg.contains("anthropic"));
                assert!(msg.contains("ollama"));
            }
            _ => panic!("expected success listing"),
        }
    }
}

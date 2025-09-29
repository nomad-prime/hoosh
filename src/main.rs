use anyhow::Result;
use clap::Parser;
#[cfg(feature = "together-ai")]
use hoosh::backends::{TogetherAiBackend, TogetherAiConfig};
use hoosh::{
    backends::{LlmBackend, MockBackend},
    cli::{Cli, Commands, ConfigAction},
    config::AppConfig,
    console::{console, init_console},
    conversation::Conversation,
    parser::MessageParser,
    permissions::PermissionManager,
    tool_executor::ToolExecutor,
    tools::ToolRegistry,
};
use std::io::{self, Write};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load config to get configured verbosity level
    let config = AppConfig::load().unwrap_or_default();

    // Initialize console with effective verbosity (CLI takes precedence over config)
    let effective_verbosity = cli.get_effective_verbosity(config.get_verbosity());
    init_console(effective_verbosity);

    match cli.command {
        Commands::Chat {
            backend,
            add_dir,
            skip_permissions,
            message,
        } => {
            handle_chat(backend, add_dir, skip_permissions, message, &config).await?;
        }
        Commands::Config { action } => {
            handle_config(action)?;
        }
    }

    Ok(())
}

async fn handle_chat(
    backend_name: Option<String>,
    add_dirs: Vec<String>,
    skip_permissions: bool,
    message: Option<String>,
    config: &AppConfig,
) -> Result<()> {
    let backend_name = backend_name.unwrap_or(config.default_backend.clone());

    let backend: Box<dyn LlmBackend> = create_backend(&backend_name, &config)?;

    let working_dir = if !add_dirs.is_empty() {
        PathBuf::from(&add_dirs[0])
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    let parser = MessageParser::with_working_directory(working_dir.clone());
    let permission_manager = PermissionManager::new().with_skip_permissions(skip_permissions);

    let tool_registry = ToolExecutor::create_tool_registry_with_working_dir(working_dir.clone());

    let system_prompt = config.load_system_prompt()?;

    if let Some(msg) = message {
        let expanded_message = match parser.expand_message(&msg).await {
            Ok(expanded) => {
                if expanded != msg {
                    console().verbose("Expanded file references in message...");
                }
                expanded
            }
            Err(e) => {
                console().warning(&format!("Error expanding file references: {}", e));
                msg // Use original message if expansion fails
            }
        };

        let mut conversation = Conversation::new();
        if let Some(system_content) = system_prompt {
            conversation.add_system_message(system_content);
        }
        conversation.add_user_message(expanded_message);

        let tool_executor = ToolExecutor::new(tool_registry.clone(), permission_manager);

        handle_conversation_turn(&backend, &mut conversation, &tool_registry, &tool_executor)
            .await?;
    } else {
        interactive_chat(backend, parser, permission_manager, tool_registry, config).await?;
    }

    Ok(())
}

fn print_help(tool_registry: &ToolRegistry) {
    console().help_header();
    console().plain("  @filename       - Reference a file (e.g., @src/main.rs)");
    console().plain("  @filename:10-20 - Reference specific lines of a file");
    console().plain("  /help           - Show this help");
    console().plain("  /tools          - List available tools");
    console().plain("  exit, quit, q   - Exit the chat");
    console().newline();
    console().plain(&format!("🔧 Available tools: {}", tool_registry.list_tools().len()));
    for (name, description) in tool_registry.list_tools() {
        console().plain(&format!("  • {}: {}", name, description));
    }
    console().newline();
}

fn print_available_tools(tool_registry: &ToolRegistry) {
    console().tools_header();
    for (name, description) in tool_registry.list_tools() {
        console().plain(&format!("  • {}: {}", name, description));
    }
    console().newline();
}

fn create_backend(backend_name: &str, config: &AppConfig) -> Result<Box<dyn LlmBackend>> {
    match backend_name {
        "mock" => {
            let _ = config; // Suppress unused warning when together-ai feature is disabled
            Ok(Box::new(MockBackend::new()))
        }
        #[cfg(feature = "together-ai")]
        "together_ai" => {
            let backend_config = config.get_backend_config("together_ai");
            let api_key = backend_config
                .and_then(|c| c.api_key.clone())
                .unwrap_or_default();
            let model = backend_config
                .and_then(|c| c.model.clone())
                .unwrap_or_else(|| "meta-llama/Llama-2-7b-chat-hf".to_string());
            let base_url = backend_config
                .and_then(|c| c.base_url.clone())
                .unwrap_or_else(|| "https://api.together.xyz/v1".to_string());

            let together_config = TogetherAiConfig {
                api_key,
                model,
                base_url,
            };

            Ok(Box::new(TogetherAiBackend::new(together_config)?))
        }
        _ => {
            #[cfg(feature = "together-ai")]
            let available = "mock, together_ai";
            #[cfg(not(feature = "together-ai"))]
            let available =
                "mock (together_ai requires Rust 1.82+ - enable with --features together-ai)";
            anyhow::bail!(
                "Unknown backend: {}. Available backends: {}",
                backend_name,
                available
            );
        }
    }
}

/// Handle a single conversation turn with tool support
async fn handle_conversation_turn(
    backend: &Box<dyn LlmBackend>,
    conversation: &mut Conversation,
    tool_registry: &ToolRegistry,
    tool_executor: &ToolExecutor,
) -> Result<()> {
    console().thinking();

    handle_conversation_step(backend, conversation, tool_registry, tool_executor, 0).await
}

/// Recursively handle conversation steps with tool calls
fn handle_conversation_step<'a>(
    backend: &'a Box<dyn LlmBackend>,
    conversation: &'a mut Conversation,
    tool_registry: &'a ToolRegistry,
    tool_executor: &'a ToolExecutor,
    step: usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + 'a>> {
    Box::pin(async move {
    const MAX_STEPS: usize = 30;
    if step >= MAX_STEPS {
        console().max_steps_reached(MAX_STEPS);
        return Ok(());
    }

    let response = backend
        .send_message_with_tools(conversation, tool_registry)
        .await?;

    if let Some(tool_calls) = response.tool_calls {
        if let Some(ref content) = response.content {
            console().verbose(&format!("LLM Response: {}", content));
        }

        conversation.add_assistant_message(response.content, Some(tool_calls.clone()));

        if step == 0 {
            console().executing_tools();
        } else {
            console().executing_more_tools();
        }

        let tool_results = tool_executor.execute_tool_calls(&tool_calls).await;

        for tool_result in tool_results {
            if let Ok(ref result) = tool_result.result {
                console().verbose(&format!("Tool '{}' result: {}",
                    tool_result.tool_name,
                    if result.len() > 200 {
                        format!("{}...", &result[..200])
                    } else {
                        result.clone()
                    }
                ));
            }
            conversation.add_tool_result(tool_result);
        }

        handle_conversation_step(backend, conversation, tool_registry, tool_executor, step + 1).await
    } else if let Some(content) = response.content {
        console().plain(&format!("{}", content));
        console().newline();
        conversation.add_assistant_message(Some(content), None);
        Ok(())
    } else {
        console().warning("No response received.");
        Ok(())
    }
    })
}

async fn interactive_chat(
    backend: Box<dyn LlmBackend>,
    parser: MessageParser,
    permission_manager: PermissionManager,
    tool_registry: ToolRegistry,
    config: &AppConfig,
) -> Result<()> {
    console().welcome(backend.backend_name());
    console().file_system_enabled();
    if !permission_manager.is_enforcing() {
        console().permissions_disabled();
    }
    console().plain("Type 'exit', 'quit', or Ctrl+C to quit.");
    console().newline();

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    let mut conversation = Conversation::new();
    let system_prompt_content = config.load_system_prompt()?;

    if let Some(system_content) = system_prompt_content {
        conversation.add_system_message(system_content);
    }
    let tool_executor = ToolExecutor::new(tool_registry.clone(), permission_manager);

    loop {
        print!("🔸 ");
        io::stdout().flush()?;

        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF
            Ok(_) => {
                let input = line.trim();

                if input.is_empty() {
                    continue;
                }

                if matches!(input, "exit" | "quit" | "q") {
                    console().goodbye();
                    break;
                }

                if input.starts_with("/help") {
                    print_help(&tool_registry);
                    continue;
                }

                if input.starts_with("/tools") {
                    print_available_tools(&tool_registry);
                    continue;
                }

                // Expand @-file references
                let expanded_input = match parser.expand_message(input).await {
                    Ok(expanded) => {
                        if expanded != input {
                            console().file_references_found();
                        }
                        expanded
                    }
                    Err(e) => {
                        console().warning(&format!("Error expanding file references: {}", e));
                        input.to_string() // Use original input if expansion fails
                    }
                };

                // Add user message to conversation
                conversation.add_user_message(expanded_input);

                // Handle the conversation turn
                if let Err(e) = handle_conversation_turn(
                    &backend,
                    &mut conversation,
                    &tool_registry,
                    &tool_executor,
                )
                .await
                {
                    console().error(&format!("Error: {}", e));
                }
            }
            Err(e) => {
                console().error(&format!("Error reading input: {}", e));
                break;
            }
        }
    }

    Ok(())
}

fn handle_config(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => {
            let config = AppConfig::load()?;
            console().plain(&format!("default_backend = \"{}\"", config.default_backend));
            if let Some(ref verbosity) = config.verbosity {
                console().plain(&format!("verbosity = \"{}\"", verbosity));
            }
            if let Some(ref system_prompt) = config.system_prompt {
                console().plain(&format!("system_prompt = \"{}\"", system_prompt));
            }

            for (backend_name, backend_config) in &config.backends {
                console().newline();
                console().plain(&format!("[{}]", backend_name));
                if let Some(ref api_key) = backend_config.api_key {
                    let masked_key = if api_key.len() > 8 {
                        format!("{}...{}", &api_key[..4], &api_key[api_key.len() - 4..])
                    } else {
                        "***".to_string()
                    };
                    console().plain(&format!("api_key = \"{}\"", masked_key));
                }
                if let Some(ref model) = backend_config.model {
                    console().plain(&format!("model = \"{}\"", model));
                }
                if let Some(ref base_url) = backend_config.base_url {
                    console().plain(&format!("base_url = \"{}\"", base_url));
                }
            }
        }
        ConfigAction::Set { key, value } => {
            let mut config = AppConfig::load()?;

            if key == "default_backend" {
                config.default_backend = value;
                config.save()?;
                console().success("Configuration updated successfully");
            } else if key == "verbosity" {
                match value.as_str() {
                    "quiet" | "normal" | "verbose" | "debug" => {
                        config.verbosity = Some(value);
                        config.save()?;
                        console().success("Verbosity configuration updated successfully");
                    }
                    _ => {
                        console().error("Invalid verbosity level. Valid options: quiet, normal, verbose, debug");
                        return Ok(());
                    }
                }
            } else if key == "system_prompt" {
                config.system_prompt = Some(value);
                config.save()?;
                console().success("System prompt path updated successfully");
            } else if let Some((backend_name, setting_key)) = key.split_once('_') {
                if matches!(backend_name, "together")
                    && matches!(setting_key, "ai_api_key" | "ai_model" | "ai_base_url")
                {
                    // Handle together_ai_* keys by splitting further
                    if setting_key.starts_with("ai_") {
                        let actual_key = &setting_key[3..]; // Remove "ai_" prefix
                        config.update_backend_setting("together_ai", actual_key, value)?;
                        config.save()?;
                        console().success("Backend configuration updated successfully");
                    } else {
                        console().error(&format!("Unknown config key: {}. Available keys: default_backend, verbosity, together_ai_api_key, together_ai_model, together_ai_base_url", key));
                    }
                } else {
                    console().error(&format!("Unknown config key: {}. Available keys: default_backend, verbosity, together_ai_api_key, together_ai_model, together_ai_base_url", key));
                }
            } else {
                console().error(&format!("Unknown config key: {}. Available keys: default_backend, verbosity, together_ai_api_key, together_ai_model, together_ai_base_url", key));
            }
        }
    }
    Ok(())
}


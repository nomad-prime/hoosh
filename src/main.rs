use anyhow::Result;
use clap::Parser;
use futures_util::StreamExt;
#[cfg(feature = "together-ai")]
use hoosh::backends::{TogetherAiBackend, TogetherAiConfig};
use hoosh::{
    backends::{LlmBackend, MockBackend},
    cli::{Cli, Commands, ConfigAction},
    config::AppConfig,
    parser::MessageParser,
    permissions::PermissionManager,
    tools::{Tool, ToolRegistry},
};
use std::io::{self, Write};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Chat {
            backend,
            add_dir,
            skip_permissions,
            message,
        } => {
            handle_chat(backend, add_dir, skip_permissions, message).await?;
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
) -> Result<()> {
    let config = AppConfig::load()?;
    let backend_name = backend_name.unwrap_or(config.default_backend.clone());

    let backend: Box<dyn LlmBackend> = create_backend(&backend_name, &config)?;

    // Set up working directory (use first add_dir or current directory)
    let working_dir = if !add_dirs.is_empty() {
        PathBuf::from(&add_dirs[0])
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    // Create message parser and permission manager
    let parser = MessageParser::with_working_directory(working_dir.clone());
    let permission_manager = PermissionManager::new().with_skip_permissions(skip_permissions);

    // Create tool registry
    let tool_registry = ToolRegistry::default();

    if let Some(msg) = message {
        // Expand @-file references
        let expanded_message = match parser.expand_message(&msg).await {
            Ok(expanded) => {
                if expanded != msg {
                    println!("📁 Expanded file references in message...\n");
                }
                expanded
            }
            Err(e) => {
                eprintln!("⚠️  Error expanding file references: {}", e);
                msg // Use original message if expansion fails
            }
        };

        println!("🤖 Thinking...\n");
        let mut stream = backend.stream_message(&expanded_message).await?;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    print!("{}", chunk);
                    io::stdout().flush()?;
                }
                Err(e) => {
                    eprintln!("Stream error: {}", e);
                    break;
                }
            }
        }
        println!("\n");
    } else {
        interactive_chat(backend, parser, permission_manager, tool_registry).await?;
    }

    Ok(())
}

fn print_help(tool_registry: &ToolRegistry) {
    println!("📚 Hoosh Help:");
    println!("  @filename       - Reference a file (e.g., @src/main.rs)");
    println!("  @filename:10-20 - Reference specific lines of a file");
    println!("  /help           - Show this help");
    println!("  /tools          - List available tools");
    println!("  exit, quit, q   - Exit the chat");
    println!();
    println!("🔧 Available tools: {}", tool_registry.list_tools().len());
    for (name, description) in tool_registry.list_tools() {
        println!("  • {}: {}", name, description);
    }
    println!();
}

fn print_available_tools(tool_registry: &ToolRegistry) {
    println!("🔧 Available Tools:");
    for (name, description) in tool_registry.list_tools() {
        println!("  • {}: {}", name, description);
    }
    println!();
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

async fn interactive_chat(
    backend: Box<dyn LlmBackend>,
    parser: MessageParser,
    permission_manager: PermissionManager,
    tool_registry: ToolRegistry,
) -> Result<()> {
    println!(
        "🚀 Welcome to hoosh! Using backend: {}",
        backend.backend_name()
    );
    println!("📁 File system integration enabled - use @filename to reference files");
    if !permission_manager.is_enforcing() {
        println!("⚠️  Permission checks disabled (--skip-permissions)");
    }
    println!("Type 'exit', 'quit', or Ctrl+C to quit.\n");

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

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
                    println!("👋 Goodbye!");
                    break;
                }

                // Check for special commands
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
                            println!("📁 Found file references, expanding...\n");
                        }
                        expanded
                    }
                    Err(e) => {
                        eprintln!("⚠️  Error expanding file references: {}\n", e);
                        input.to_string() // Use original input if expansion fails
                    }
                };

                println!("🤖 ");

                match backend.stream_message(&expanded_input).await {
                    Ok(mut stream) => {
                        while let Some(chunk_result) = stream.next().await {
                            match chunk_result {
                                Ok(chunk) => {
                                    print!("{}", chunk);
                                    io::stdout().flush()?;
                                }
                                Err(e) => {
                                    eprintln!("\nStream error: {}", e);
                                    break;
                                }
                            }
                        }
                        println!("\n");
                    }
                    Err(e) => {
                        eprintln!("Error: {}\n", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading input: {}", e);
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
            println!("default_backend = \"{}\"", config.default_backend);

            for (backend_name, backend_config) in &config.backends {
                println!("\n[{}]", backend_name);
                if let Some(ref api_key) = backend_config.api_key {
                    let masked_key = if api_key.len() > 8 {
                        format!("{}...{}", &api_key[..4], &api_key[api_key.len() - 4..])
                    } else {
                        "***".to_string()
                    };
                    println!("api_key = \"{}\"", masked_key);
                }
                if let Some(ref model) = backend_config.model {
                    println!("model = \"{}\"", model);
                }
                if let Some(ref base_url) = backend_config.base_url {
                    println!("base_url = \"{}\"", base_url);
                }
            }
        }
        ConfigAction::Set { key, value } => {
            let mut config = AppConfig::load()?;

            if key == "default_backend" {
                config.default_backend = value;
                config.save()?;
                println!("Configuration updated successfully");
            } else if let Some((backend_name, setting_key)) = key.split_once('_') {
                if matches!(backend_name, "together")
                    && matches!(setting_key, "ai_api_key" | "ai_model" | "ai_base_url")
                {
                    // Handle together_ai_* keys by splitting further
                    if setting_key.starts_with("ai_") {
                        let actual_key = &setting_key[3..]; // Remove "ai_" prefix
                        config.update_backend_setting("together_ai", actual_key, value)?;
                        config.save()?;
                        println!("Backend configuration updated successfully");
                    } else {
                        eprintln!("Unknown config key: {}. Available keys: default_backend, together_ai_api_key, together_ai_model, together_ai_base_url", key);
                    }
                } else {
                    eprintln!("Unknown config key: {}. Available keys: default_backend, together_ai_api_key, together_ai_model, together_ai_base_url", key);
                }
            } else {
                eprintln!("Unknown config key: {}. Available keys: default_backend, together_ai_api_key, together_ai_model, together_ai_base_url", key);
            }
        }
    }
    Ok(())
}

use crate::tools::ToolRegistry;
use anyhow::Result;
use std::io::{self, Write};

pub fn prompt_yes_no(message: &str) -> Result<bool> {
    eprintln!("\n┌─────────────────────────────────────────┐");
    eprintln!("│ {:<39} │", message);
    eprintln!("└─────────────────────────────────────────┘");
    eprint!("  (y/n): ");
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().eq_ignore_ascii_case("y"))
}

pub fn prompt_choice(message: &str, choices: &[&str]) -> Result<usize> {
    eprintln!("\n┌─────────────────────────────────────────┐");
    eprintln!("│ {:<39} │", message);
    eprintln!("├─────────────────────────────────────────┤");

    for (i, choice) in choices.iter().enumerate() {
        eprintln!("│ {}. {:<37} │", i + 1, choice);
    }

    eprintln!("└─────────────────────────────────────────┘");
    eprint!("  Choice (1-{}): ", choices.len());
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let choice = input.trim().parse::<usize>()?;
    if choice < 1 || choice > choices.len() {
        anyhow::bail!("Invalid choice");
    }

    Ok(choice - 1)
}

pub fn display_message(message: &str) {
    eprintln!("\n┌─────────────────────────────────────────┐");
    for line in message.lines() {
        eprintln!("│ {:<39} │", line);
    }
    eprintln!("└─────────────────────────────────────────┘\n");
}

pub fn handle_initial_permissions(
    working_dir: &std::path::Path,
    tool_registry: &ToolRegistry,
) -> Result<bool> {
    use crate::permissions::storage::PermissionsFile;

    let permissions_path = PermissionsFile::get_permissions_path(working_dir);
    if permissions_path.exists() {
        return Ok(true);
    }

    eprintln!("\nWorking directory: {}", working_dir.display());
    eprintln!("\nHoosh needs permissions to:");
    eprintln!("  • Read and write files in the working directory");
    eprintln!("  • Execute shell commands");
    eprintln!(
        "  • Access tools: {}",
        tool_registry
            .list_tools()
            .iter()
            .take(5)
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
            .join(", ")
    );

    eprintln!("\n┌─────────────────────────────────────────┐");
    eprintln!("│ Grant initial permissions?              │");
    eprintln!("└─────────────────────────────────────────┘");
    eprintln!("  1. Grant all (recommended)");
    eprintln!("  2. Prompt for each action");
    eprintln!("  3. Cancel");
    eprint!("\n  Choice (1-3): ");
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    match input.trim() {
        "1" => {
            eprintln!("\n✓ All permissions granted");
            Ok(true)
        }
        "2" => {
            eprintln!("\n✓ Will prompt for each action");
            Ok(false)
        }
        "3" | "" => {
            eprintln!("\n✗ Cancelled");
            anyhow::bail!("User cancelled permission setup")
        }
        _ => {
            eprintln!("\n✗ Invalid choice");
            anyhow::bail!("Invalid choice")
        }
    }
}

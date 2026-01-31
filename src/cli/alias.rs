use crate::cli::shell_setup::{ShellType, detect_shell, install_shell_alias};
use crate::console::console;
use anyhow::Result;

pub fn handle_alias_install() -> Result<()> {
    console().info("─── Shell Integration Setup ───");
    console().info("This will add the @hoosh alias to your shell configuration.");
    console().info("Usage: @hoosh \"your message here\"");
    console().info("");

    match detect_shell() {
        Ok(shell_type) => {
            console().info(&format!("Detected shell: {}", shell_type_name(&shell_type)));
            install_shell_alias(shell_type)?;
        }
        Err(e) => {
            console().error(&format!("Could not detect shell: {}", e));
            console().info("Supported shells: bash, zsh, fish");
            return Err(e);
        }
    }

    Ok(())
}

fn shell_type_name(shell_type: &ShellType) -> &'static str {
    match shell_type {
        ShellType::Bash => "Bash",
        ShellType::Zsh => "Zsh",
        ShellType::Fish => "Fish",
        #[cfg(windows)]
        ShellType::PowerShell => "PowerShell",
    }
}

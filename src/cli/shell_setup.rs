// Shell setup module for @hoosh alias installation

use anyhow::{Context, Result, anyhow};
use std::path::PathBuf;

/// Supported shell types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    #[cfg(windows)]
    PowerShell,
}

/// Detect the current shell from environment
pub fn detect_shell() -> Result<ShellType> {
    // Method 1: Check SHELL environment variable
    if let Ok(shell_path) = std::env::var("SHELL") {
        if shell_path.contains("zsh") {
            return Ok(ShellType::Zsh);
        } else if shell_path.contains("bash") {
            return Ok(ShellType::Bash);
        } else if shell_path.contains("fish") {
            return Ok(ShellType::Fish);
        }
    }

    // Method 2: Check for shell-specific environment variables
    if std::env::var("ZSH_VERSION").is_ok() {
        return Ok(ShellType::Zsh);
    }
    if std::env::var("BASH_VERSION").is_ok() {
        return Ok(ShellType::Bash);
    }

    #[cfg(windows)]
    {
        // On Windows, default to PowerShell if no other shell detected
        return Ok(ShellType::PowerShell);
    }

    Err(anyhow!(
        "Could not detect shell type. Please specify manually."
    ))
}

pub fn get_shell_config_path(shell_type: ShellType) -> Result<PathBuf> {
    let home = dirs::home_dir().context("Failed to get home directory")?;

    match shell_type {
        ShellType::Bash => Ok(home.join(".bashrc")),
        ShellType::Zsh => Ok(home.join(".zshrc")),
        ShellType::Fish => {
            let fish_dir = home.join(".config").join("fish").join("functions");
            Ok(fish_dir.join("@hoosh.fish"))
        }
        #[cfg(windows)]
        ShellType::PowerShell => {
            if let Ok(profile) = std::env::var("PROFILE") {
                Ok(PathBuf::from(profile))
            } else {
                Ok(home
                    .join("Documents")
                    .join("PowerShell")
                    .join("Microsoft.PowerShell_profile.ps1"))
            }
        }
    }
}

pub fn generate_shell_function(shell_type: ShellType) -> String {
    match shell_type {
        ShellType::Bash | ShellType::Zsh => r#"# Added by hoosh setup
@hoosh() {
    export PPID="$$"
    hoosh agent --mode tagged "$@"
}
"#
        .to_string(),
        ShellType::Fish => r#"# Added by hoosh setup
function @hoosh --description 'Hoosh AI assistant in tagged mode'
    set -x PPID %self
    hoosh agent --mode tagged $argv
end
"#
        .to_string(),
        #[cfg(windows)]
        ShellType::PowerShell => r#"# Added by hoosh setup
function @hoosh {
    $env:PID = $PID
    hoosh agent --mode tagged $args
}
"#
        .to_string(),
    }
}

pub fn install_shell_alias(shell_type: ShellType) -> Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let config_path = get_shell_config_path(shell_type)?;

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create config directory")?;
    }

    if config_path.exists() {
        let content =
            std::fs::read_to_string(&config_path).context("Failed to read shell config file")?;
        if content.contains("@hoosh") {
            eprintln!("⚠️  @hoosh already defined in {}", config_path.display());
            eprintln!("   Skipping installation. Remove existing definition to reinstall.");
            return Ok(());
        }
    }

    let function_def = generate_shell_function(shell_type);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config_path)
        .context("Failed to open shell config file")?;

    writeln!(file, "\n{}", function_def).context("Failed to write to shell config file")?;

    eprintln!("✅ Installed @hoosh function in {}", config_path.display());
    eprintln!(
        "   Run 'source {}' or restart terminal to activate",
        config_path.display()
    );

    Ok(())
}

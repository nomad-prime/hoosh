use colored::Colorize;
use std::fmt;
use std::sync::{Arc, OnceLock};

/// Verbosity levels for console output
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum VerbosityLevel {
    /// Only show errors
    Quiet = 0,
    /// Normal output (default)
    #[default]
    Normal = 1,
    /// Verbose output with additional info
    Verbose = 2,
    /// Debug output with detailed information
    Debug = 3,
}

/// Message types for different console outputs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// User input
    UserInput,
    /// Assistant thought/commentary
    AssistantThought,
    /// Tool execution indicator
    ToolExecution,
    /// Tool result
    ToolResult,
    /// Final response from assistant
    FinalResponse,
    /// Thinking indicator
    Thinking,
    /// Error message
    Error,
    /// Warning message
    Warning,
    /// Success message
    Success,
}

impl fmt::Display for VerbosityLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerbosityLevel::Quiet => write!(f, "quiet"),
            VerbosityLevel::Normal => write!(f, "normal"),
            VerbosityLevel::Verbose => write!(f, "verbose"),
            VerbosityLevel::Debug => write!(f, "debug"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Console {
    verbosity: VerbosityLevel,
}

impl Console {
    pub fn new(verbosity: VerbosityLevel) -> Self {
        Self { verbosity }
    }

    pub fn set_verbosity(&mut self, verbosity: VerbosityLevel) {
        self.verbosity = verbosity;
    }

    pub fn verbosity(&self) -> VerbosityLevel {
        self.verbosity
    }

    fn should_show(&self, level: VerbosityLevel) -> bool {
        self.verbosity >= level
    }

    pub fn error(&self, message: &str) {
        if self.verbosity > VerbosityLevel::Quiet {
            eprintln!("❌ {}", message);
        }
    }

    pub fn warning(&self, message: &str) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("⚠️  {}", message);
        }
    }

    pub fn info(&self, message: &str) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("ℹ️  {}", message);
        }
    }

    pub fn success(&self, message: &str) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("✅ {}", message);
        }
    }

    pub fn thinking(&self) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("{}", "🔄 Thinking...".dimmed());
        }
    }

    pub fn message(&self, msg_type: MessageType, content: &str) {
        if !self.should_show(VerbosityLevel::Normal) {
            return;
        }

        match msg_type {
            MessageType::Thinking => println!("{}", "🔄 Thinking...".dimmed()),
            MessageType::ToolExecution => println!("{}", "↘ Executing tools...".dimmed()),
            MessageType::AssistantThought => {
                if !content.is_empty() {
                    println!("\n{} {}", "•".dimmed(), content);
                }
            }
            MessageType::ToolResult => {
                if !content.is_empty() {
                    println!("{}", content);
                }
            }
            MessageType::FinalResponse => {
                if !content.is_empty() {
                    println!("{}", content);
                }
            }
            MessageType::Error => println!("❌ {}", content),
            MessageType::Warning => println!("⚠️  {}", content),
            MessageType::Success => println!("✅ {}", content),
            MessageType::UserInput => println!("> {}", content),
        }
    }

    pub fn tool_call(&self, tool_name: &str, args_summary: &str) {
        if !self.should_show(VerbosityLevel::Normal) {
            return;
        }
        println!(
            "{} {}{}{}",
            "⏺".dimmed(),
            tool_name.green(),
            "(".dimmed(),
            args_summary.dimmed()
        );
    }

    pub fn tool_result_summary(&self, summary: &str) {
        if !self.should_show(VerbosityLevel::Normal) {
            return;
        }
        println!("  {} {}", "⎿".dimmed(), summary.dimmed());
    }

    pub fn tool_result(&self, tool_name: &str, result: &str, max_length: usize) {
        if !self.should_show(VerbosityLevel::Normal) {
            return;
        }

        let truncated = if result.len() > max_length {
            let mut s = result.chars().take(max_length).collect::<String>();
            s.push_str("...");
            s
        } else {
            result.to_string()
        };

        println!(
            "{} {} {}",
            "Tool".dimmed(),
            format!("'{}'", tool_name).cyan(),
            "result:".dimmed()
        );

        for (i, line) in truncated.lines().enumerate() {
            if i >= 15 {
                println!("  {}", "...".dimmed());
                break;
            }
            println!("  {}", line);
        }
    }

    pub fn executing_tools_arrow(&self) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("{}", "↘ Executing tools...".dimmed());
        }
    }

    pub fn executing_more_tools_arrow(&self) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("{}", "↘ Executing more tools...".dimmed());
        }
    }

    pub fn assistant_thought(&self, content: &str) {
        if self.should_show(VerbosityLevel::Normal) && !content.is_empty() {
            println!("{} {}", "•".dimmed(), content);
        }
    }

    pub fn executing_tools(&self) {
        if self.should_show(VerbosityLevel::Debug) {
            println!("🔧 Executing tools...");
        }
    }

    pub fn executing_more_tools(&self) {
        if self.should_show(VerbosityLevel::Debug) {
            println!("🔧 Executing more tools...");
        }
    }

    pub fn verbose(&self, message: &str) {
        if self.should_show(VerbosityLevel::Verbose) {
            println!("{}", message);
        }
    }

    pub fn debug(&self, message: &str) {
        if self.should_show(VerbosityLevel::Debug) {
            println!("🐛 DEBUG: {}", message);
        }
    }

    pub fn plain(&self, message: &str) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("{}", message);
        }
    }

    pub fn print(&self, message: &str) {
        if self.should_show(VerbosityLevel::Normal) {
            print!("{}", message);
        }
    }

    pub fn newline(&self) {
        if self.should_show(VerbosityLevel::Normal) {
            println!();
        }
    }

    pub fn welcome(&self, backend_name: &str) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("🚀 Welcome to hoosh! Using backend: {}", backend_name);
        }
    }

    pub fn file_system_enabled(&self) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("📁 File system integration enabled - use @filename to reference files");
        }
    }

    pub fn permissions_disabled(&self) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("⚠️ Permission checks disabled (--skip-permissions)");
        }
    }

    pub fn file_references_found(&self) {
        if self.should_show(VerbosityLevel::Verbose) {
            println!("📁 Found file references, expanding...");
        }
    }

    pub fn goodbye(&self) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("👋 Goodbye!");
        }
    }

    pub fn help_header(&self) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("📚 Hoosh Help:");
        }
    }

    pub fn tools_header(&self) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("🔧 Available Tools:");
        }
    }

    pub fn max_steps_reached(&self, max_steps: usize) {
        if self.should_show(VerbosityLevel::Normal) {
            println!(
                "⚠️ Maximum conversation steps ({}) reached, stopping.",
                max_steps
            );
        }
    }
}

static GLOBAL_CONSOLE: OnceLock<Arc<Console>> = OnceLock::new();

pub fn init_console(verbosity: VerbosityLevel) {
    let _ = GLOBAL_CONSOLE.set(Arc::new(Console::new(verbosity)));
}

pub fn console() -> Arc<Console> {
    GLOBAL_CONSOLE
        .get()
        .expect("Console not initialized - call init_console() first")
        .clone()
}

impl Default for Console {
    fn default() -> Self {
        Self {
            verbosity: VerbosityLevel::Normal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verbosity_levels() {
        assert!(VerbosityLevel::Quiet < VerbosityLevel::Normal);
        assert!(VerbosityLevel::Normal < VerbosityLevel::Verbose);
        assert!(VerbosityLevel::Verbose < VerbosityLevel::Debug);
    }

    #[test]
    fn test_console_should_show() {
        let console = Console::new(VerbosityLevel::Normal);

        assert!(!console.should_show(VerbosityLevel::Verbose));
        assert!(console.should_show(VerbosityLevel::Normal));
        assert!(!console.should_show(VerbosityLevel::Debug));
    }

    #[test]
    fn test_verbosity_display() {
        assert_eq!(VerbosityLevel::Quiet.to_string(), "quiet");
        assert_eq!(VerbosityLevel::Normal.to_string(), "normal");
        assert_eq!(VerbosityLevel::Verbose.to_string(), "verbose");
        assert_eq!(VerbosityLevel::Debug.to_string(), "debug");
    }
}

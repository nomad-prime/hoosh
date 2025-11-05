use crate::console::VerbosityLevel;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    /// Increase verbosity (-v verbose, -vv debug)
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Quiet mode - only show errors
    #[arg(short = 'q', long = "quiet", conflicts_with = "verbose")]
    pub quiet: bool,

    /// Backend to use for chat
    #[arg(short, long)]
    pub backend: Option<String>,

    /// Add directories to the context
    #[arg(long)]
    pub add_dir: Vec<String>,

    /// Skip permission checks
    #[arg(long)]
    pub skip_permissions: bool,

    /// Continue the last conversation
    #[arg(long = "continue")]
    pub continue_last: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    Conversations {
        #[command(subcommand)]
        action: ConversationsAction,
    },
}

#[derive(Subcommand)]
pub enum ConversationsAction {
    List,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    Show,
    Set { key: String, value: String },
}

impl Cli {
    pub fn get_verbosity(&self) -> VerbosityLevel {
        if self.quiet {
            VerbosityLevel::Quiet
        } else {
            match self.verbose {
                0 => VerbosityLevel::Normal,
                1 => VerbosityLevel::Verbose,
                _ => VerbosityLevel::Debug,
            }
        }
    }

    pub fn get_effective_verbosity(&self, config_verbosity: VerbosityLevel) -> VerbosityLevel {
        if self.quiet || self.verbose > 0 {
            // CLI verbosity specified, use it
            self.get_verbosity()
        } else {
            // No CLI verbosity specified, use config
            config_verbosity
        }
    }
}

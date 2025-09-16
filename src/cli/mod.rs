use clap::{Parser, Subcommand};
use crate::console::VerbosityLevel;

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    /// Increase verbosity (-v verbose, -vv debug)
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Quiet mode - only show errors
    #[arg(short = 'q', long = "quiet", conflicts_with = "verbose")]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Chat {
        #[arg(short, long)]
        backend: Option<String>,
        #[arg(long)]
        add_dir: Vec<String>,
        #[arg(long)]
        skip_permissions: bool,
        message: Option<String>,
    },
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
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

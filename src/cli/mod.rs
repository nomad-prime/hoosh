use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
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

use clap::{Parser, Subcommand};

pub mod handlers;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Open the TUI music player
    Play,
    /// Update the local music index
    Index,
    /// Scan for and optionally remove duplicate local tracks
    Dedup {
        /// Actually delete the duplicate files (dry run by default)
        #[arg(long)]
        delete: bool,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

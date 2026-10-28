use anyhow::Result;
use clap::{Parser, Subcommand};

mod dox;
mod map;
mod rlm;

#[derive(Parser)]
#[command(name = "aoc")]
#[command(about = "Agent Ops Cockpit CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze large codebases (RLM)
    Rlm {
        #[command(subcommand)]
        action: rlm::RlmCommand,
    },
    /// Map sparse AGENTS.md context contracts
    Dox {
        #[command(subcommand)]
        action: dox::DoxCommand,
    },
    /// Build and serve agent-authored project maps
    #[command(alias = "see")]
    Map {
        #[command(subcommand)]
        action: map::MapCommand,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Dox { action } => dox::handle_dox_command(action),
        Commands::Rlm { action } => rlm::handle_rlm_command(action),
        Commands::Map { action } => map::handle_map_command(action),
    }
}

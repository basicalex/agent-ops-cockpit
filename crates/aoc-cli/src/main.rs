use anyhow::Result;
use clap::{Parser, Subcommand};

mod insight;
mod map;
mod overseer;
mod rlm;
mod task;

#[derive(Parser)]
#[command(name = "aoc")]
#[command(about = "Agent Ops Cockpit CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage tasks
    Task {
        #[command(subcommand)]
        action: task::TaskCommand,
    },
    /// Manage memory
    Mem {
        #[command(subcommand)]
        action: MemCommands,
    },
    /// Analyze large codebases (RLM)
    Rlm {
        #[command(subcommand)]
        action: rlm::RlmCommand,
    },
    /// Query Mind-backed insight retrieval, provenance, and runtime status
    Insight {
        #[command(subcommand)]
        action: insight::InsightCommand,
    },
    /// Inspect and steer the session overseer control plane
    Overseer {
        #[command(subcommand)]
        action: overseer::OverseerCommand,
    },
    /// Build and serve agent-authored project maps
    #[command(alias = "see")]
    Map {
        #[command(subcommand)]
        action: map::MapCommand,
    },
}

#[derive(Subcommand)]
enum MemCommands {
    Add { content: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Task { action } => task::handle_task_command(action),
        Commands::Mem { action } => match action {
            MemCommands::Add { content } => {
                println!("Adding memory: {}", content);
                Ok(())
            }
        },
        Commands::Rlm { action } => rlm::handle_rlm_command(action),
        Commands::Insight { action } => insight::handle_insight_command(action),
        Commands::Overseer { action } => overseer::handle_overseer_command(action),
        Commands::Map { action } => map::handle_map_command(action),
    }
}

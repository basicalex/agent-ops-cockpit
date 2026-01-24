use anyhow::Result;
use clap::{Parser, Subcommand};

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
    }
}

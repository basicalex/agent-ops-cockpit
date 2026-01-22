use anyhow::{Context, Result};
use aoc_core::ProjectData;
use clap::{Parser, Subcommand};
use std::fs;

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
        action: TaskCommands,
    },
    /// Manage memory
    Mem {
        #[command(subcommand)]
        action: MemCommands,
    },
}

#[derive(Subcommand)]
enum TaskCommands {
    List,
}

#[derive(Subcommand)]
enum MemCommands {
    Add { content: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Task { action } => match action {
            TaskCommands::List => {
                let root = std::env::current_dir()?;
                let tasks_path = root.join(".taskmaster/tasks/tasks.json");

                if !tasks_path.exists() {
                    println!("No tasks.json found at {:?}", tasks_path);
                    return Ok(());
                }

                let content =
                    fs::read_to_string(&tasks_path).context("Failed to read tasks.json")?;

                let data: ProjectData =
                    serde_json::from_str(&content).context("Failed to parse tasks.json")?;

                if let Some(ctx) = data.tags.get("master") {
                    println!("Found {} tasks:", ctx.tasks.len());
                    for task in &ctx.tasks {
                        println!("- [{}] {}", task.id, task.title);
                    }
                } else {
                    println!("No 'master' tag found in tasks.json");
                }
            }
        },
        Commands::Mem { action } => match action {
            MemCommands::Add { content } => {
                println!("Adding memory: {}", content);
            }
        },
    }

    Ok(())
}

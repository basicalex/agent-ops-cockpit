use anyhow::{Result, anyhow};
use clap::Parser;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Zellij session name to monitor
    #[arg(short, long)]
    session: String,

    /// Debounce interval in milliseconds
    #[arg(short, long, default_value_t = 1000)]
    debounce: u64,

    /// Interval for discovery loop in seconds
    #[arg(short, long, default_value_t = 5)]
    interval: u64,
}

struct ProjectWatcher {
    _root: PathBuf,
    _root_tag: String,
    _watcher: RecommendedWatcher,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let state_dir = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("Could not find HOME environment variable");
            PathBuf::from(home).join(".local/state")
        })
        .join("aoc");

    println!("Starting AOC Watcher for session: {}", args.session);

    let (tx, mut rx) = mpsc::channel::<PathBuf>(100);
    let active_watchers: Arc<tokio::sync::Mutex<HashMap<PathBuf, ProjectWatcher>>> =
        Arc::new(tokio::sync::Mutex::new(HashMap::new()));

    // Discovery loop
    let session_name = args.session.clone();
    let interval = args.interval;
    let watchers_clone = Arc::clone(&active_watchers);
    let tx_clone = tx.clone();

    tokio::spawn(async move {
        loop {
            if let Err(e) = discover_roots(&session_name, &state_dir, &watchers_clone, &tx_clone).await {
                eprintln!("Discovery error: {}", e);
            }

            // Also check if session still exists
            if !session_exists(&session_name) {
                println!("Session {} no longer exists. Exiting.", session_name);
                std::process::exit(0);
            }

            sleep(Duration::from_secs(interval)).await;
        }
    });

    // Debounce and Update loop
    let mut pending_updates: HashSet<PathBuf> = HashSet::new();
    let debounce_duration = Duration::from_millis(args.debounce);

    loop {
        tokio::select! {
            path = rx.recv() => {
                if let Some(path) = path {
                    pending_updates.insert(path);
                }
            }
            _ = sleep(Duration::from_millis(100)) => {
                if !pending_updates.is_empty() {
                    // Wait for debounce
                    sleep(debounce_duration).await;
                    
                    // Process all pending updates
                    let paths: Vec<PathBuf> = pending_updates.drain().collect();
                    for path in paths {
                        println!("Updating context for: {:?}", path);
                        if let Err(e) = update_context(&path).await {
                            eprintln!("Update error for {:?}: {}", path, e);
                        }
                    }
                }
            }
        }
    }
}

async fn discover_roots(
    session: &str,
    state_dir: &Path,
    watchers: &Arc<tokio::sync::Mutex<HashMap<PathBuf, ProjectWatcher>>>,
    tx: &mpsc::Sender<PathBuf>,
) -> Result<()> {
    let output = Command::new("zellij")
        .args(["-s", session, "action", "dump-layout"])
        .output()?;

    if !output.status.success() {
        return Err(anyhow!("Failed to dump layout: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let layout = String::from_utf8_lossy(&output.stdout);
    let mut found_tags = HashSet::new();

    // Simple regex-like extraction for name="aoc:<root_tag>"
    for line in layout.lines() {
        if let Some(idx) = line.find("name=\"aoc:") {
            let rest = &line[idx + 10..];
            if let Some(end_idx) = rest.find('\"') {
                let tag = &rest[..end_idx];
                if tag != "__AOC_ROOT_TAG__" {
                    found_tags.insert(tag.to_string());
                }
            }
        }
    }

    let mut watchers_lock = watchers.lock().await;
    let mut current_roots = HashSet::new();

    for tag in found_tags {
        let root_file = state_dir.join(format!("project_root.{}", tag));
        if root_file.exists() {
            let root_path_str = std::fs::read_to_string(&root_file)?;
            let root_path = PathBuf::from(root_path_str.trim());
            current_roots.insert(root_path.clone());

            if !watchers_lock.contains_key(&root_path) {
                println!("New project root discovered: {:?}", root_path);
                let tx_inner = tx.clone();
                let root_path_inner = root_path.clone();
                let root_path_for_watcher = root_path.clone();
                
                let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
                    match res {
                        Ok(event) => {
                            // Ignore changes in .aoc directory to prevent loops
                            if event.paths.iter().any(|p: &PathBuf| p.to_string_lossy().contains("/.aoc/")) {
                                return;
                            }
                            let _ = tx_inner.blocking_send(root_path_for_watcher.clone());
                        }
                        Err(e) => eprintln!("watch error: {:?}", e),
                    }
                })?;

                watcher.watch(&root_path, RecursiveMode::Recursive)?;
                
                watchers_lock.insert(root_path.clone(), ProjectWatcher {
                    _root: root_path,
                    _root_tag: tag,
                    _watcher: watcher,
                });
                
                // Trigger initial update
                let _ = tx.send(root_path_inner).await;
            }
        }
    }

    // Remove stale watchers
    watchers_lock.retain(|path, _| {
        if !current_roots.contains(path) {
            println!("Removing watcher for stale root: {:?}", path);
            false
        } else {
            true
        }
    });

    Ok(())
}

fn session_exists(session: &str) -> bool {
    let output = Command::new("zellij")
        .args(["list-sessions", "--short"])
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines().any(|l| l.trim() == session)
    } else {
        false
    }
}

async fn update_context(root: &Path) -> Result<()> {
    // We leverage aoc-init to regenerate the context file.
    // This ensures consistency and avoids duplication of complex generation logic.
    let status = Command::new("aoc-init")
        .arg(root)
        .status()?;

    if !status.success() {
        return Err(anyhow!("aoc-init failed with status: {}", status));
    }

    Ok(())
}

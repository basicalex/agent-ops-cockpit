use anyhow::Result;
use clap::{Args, Subcommand};
use globset::Glob;
use ignore::WalkBuilder;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_CHUNK_SIZE: usize = 5000;
const DEFAULT_CONTEXT_WINDOW: usize = 200;
const MAX_PEEK_RESULTS: usize = 50;
const SKIP_DIRS: [&str; 5] = [".git", "node_modules", "target", "__pycache__", ".aoc"];

#[derive(Subcommand, Debug)]
#[command(rename_all = "kebab-case")]
pub enum RlmCommand {
    Scan,
    Peek(PeekArgs),
    Chunk(ChunkArgs),
}

#[derive(Args, Debug)]
pub struct PeekArgs {
    pub query: String,
}

#[derive(Args, Debug)]
pub struct ChunkArgs {
    #[arg(long)]
    pub pattern: Option<String>,
    #[arg(long, default_value_t = DEFAULT_CHUNK_SIZE)]
    pub size: usize,
}

#[derive(Serialize)]
struct ScanStats {
    loaded: usize,
    chars: usize,
}

#[derive(Serialize)]
struct ChunkEntry {
    path: String,
    chunk: usize,
    total: usize,
    lines: String,
    content: String,
}

struct FileEntry {
    path: PathBuf,
    rel: String,
}

struct PatternMatcher {
    raw: Option<String>,
    glob: Option<globset::GlobMatcher>,
}

impl PatternMatcher {
    fn new(pattern: Option<String>) -> Self {
        let raw = pattern.filter(|value| !value.is_empty());
        let glob = raw
            .as_ref()
            .and_then(|value| Glob::new(value).ok())
            .map(|glob| glob.compile_matcher());
        Self { raw, glob }
    }

    fn is_match(&self, value: &str) -> bool {
        match &self.raw {
            None => true,
            Some(raw) => {
                if let Some(glob) = &self.glob {
                    if glob.is_match(value) {
                        return true;
                    }
                }
                value.contains(raw)
            }
        }
    }
}

struct CharIndex {
    byte_indices: Vec<usize>,
    newline_counts: Vec<usize>,
    char_count: usize,
}

impl CharIndex {
    fn new(content: &str) -> Self {
        let mut byte_indices = Vec::new();
        let mut newline_counts = Vec::new();
        let mut newlines = 0usize;

        for (byte_idx, ch) in content.char_indices() {
            byte_indices.push(byte_idx);
            newline_counts.push(newlines);
            if ch == '\n' {
                newlines += 1;
            }
        }

        byte_indices.push(content.len());
        newline_counts.push(newlines);

        let char_count = byte_indices.len().saturating_sub(1);
        Self {
            byte_indices,
            newline_counts,
            char_count,
        }
    }

    fn byte_index(&self, char_index: usize) -> usize {
        let idx = char_index.min(self.char_count);
        self.byte_indices[idx]
    }

    fn char_index_from_byte(&self, byte_index: usize) -> usize {
        match self.byte_indices.binary_search(&byte_index) {
            Ok(idx) => idx,
            Err(pos) => pos.saturating_sub(1).min(self.char_count),
        }
    }

    fn newline_count(&self, char_index: usize) -> usize {
        let idx = char_index.min(self.char_count);
        self.newline_counts[idx]
    }
}

pub fn handle_rlm_command(command: RlmCommand) -> Result<()> {
    let root = std::env::current_dir()?;

    match command {
        RlmCommand::Scan => {
            let stats = scan(&root);
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
        RlmCommand::Peek(args) => {
            let results = peek(&root, &args.query);
            for line in results {
                println!("{}", line);
            }
        }
        RlmCommand::Chunk(args) => {
            let chunks = chunk(&root, args.pattern, args.size);
            println!("{}", serde_json::to_string_pretty(&chunks)?);
        }
    }

    Ok(())
}

fn scan(root: &Path) -> ScanStats {
    let entries = collect_files(root);
    let mut loaded = 0usize;
    let mut chars = 0usize;

    for entry in entries {
        if let Some(content) = read_text_lossy(&entry.path) {
            loaded += 1;
            chars += content.chars().count();
        }
    }

    ScanStats { loaded, chars }
}

fn peek(root: &Path, query: &str) -> Vec<String> {
    let entries = collect_files(root);
    let query_lower = query.to_lowercase();
    let query_len = query.chars().count();
    let mut results = Vec::new();

    if query_lower.is_empty() {
        return results;
    }

    for entry in entries {
        if results.len() >= MAX_PEEK_RESULTS {
            break;
        }

        let Some(content) = read_text_lossy(&entry.path) else {
            continue;
        };

        let lower = content.to_lowercase();
        if !lower.contains(&query_lower) {
            continue;
        }

        let lower_index = CharIndex::new(&lower);
        let content_index = CharIndex::new(&content);
        let mut start_char = 0usize;

        while start_char <= lower_index.char_count {
            let start_byte = lower_index.byte_index(start_char);
            if start_byte > lower.len() {
                break;
            }

            let Some(offset) = lower[start_byte..].find(&query_lower) else {
                break;
            };

            let match_byte = start_byte + offset;
            let match_char = lower_index.char_index_from_byte(match_byte);
            let snippet_start = match_char.saturating_sub(DEFAULT_CONTEXT_WINDOW);
            let snippet_end =
                (match_char + query_len + DEFAULT_CONTEXT_WINDOW).min(content_index.char_count);
            let byte_start = content_index.byte_index(snippet_start);
            let byte_end = content_index.byte_index(snippet_end);
            let snippet = content[byte_start..byte_end].replace('\n', " ");

            results.push(format!("[{}]: ...{}...", entry.rel, snippet));
            if results.len() >= MAX_PEEK_RESULTS {
                break;
            }
            start_char = match_char.saturating_add(1);
        }
    }

    results
}

fn chunk(root: &Path, pattern: Option<String>, size: usize) -> Vec<ChunkEntry> {
    let entries = collect_files(root);
    let matcher = PatternMatcher::new(pattern);
    let chunk_size = size.max(1);
    let mut chunks = Vec::new();

    for entry in entries {
        if !matcher.is_match(&entry.rel) {
            continue;
        }

        let Some(content) = read_text_lossy(&entry.path) else {
            continue;
        };

        let index = CharIndex::new(&content);
        let char_count = index.char_count;
        let mut total = (char_count + chunk_size - 1) / chunk_size;
        if total == 0 {
            total = 1;
        }

        for i in 0..total {
            let start = i * chunk_size;
            let end = ((i + 1) * chunk_size).min(char_count);
            let line_start = index.newline_count(start) + 1;
            let line_end = index.newline_count(end) + 1;
            let byte_start = index.byte_index(start);
            let byte_end = index.byte_index(end);
            let content_slice = content[byte_start..byte_end].to_string();

            chunks.push(ChunkEntry {
                path: entry.rel.clone(),
                chunk: i + 1,
                total,
                lines: format!("{}-{}", line_start, line_end),
                content: content_slice,
            });
        }
    }

    chunks
}

fn collect_files(root: &Path) -> Vec<FileEntry> {
    if let Some(entries) = git_file_entries(root) {
        return entries;
    }

    let mut entries = Vec::new();
    let walker = WalkBuilder::new(root).standard_filters(true).build();

    for result in walker {
        let entry = match result {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
            continue;
        }

        let path = entry.path().to_path_buf();
        let Ok(rel_path) = path.strip_prefix(root) else {
            continue;
        };
        if should_skip(rel_path) {
            continue;
        }
        let rel = rel_path.to_string_lossy().to_string();
        entries.push(FileEntry { path, rel });
    }

    entries.sort_by(|a, b| a.rel.cmp(&b.rel));
    entries
}

fn git_file_entries(root: &Path) -> Option<Vec<FileEntry>> {
    let output = Command::new("git")
        .args(["ls-files", "-c", "-o", "--exclude-standard"])
        .current_dir(root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

    for line in stdout.lines() {
        let rel = line.trim();
        if rel.is_empty() {
            continue;
        }

        if should_skip(Path::new(rel)) {
            continue;
        }

        let path = root.join(rel);
        entries.push(FileEntry {
            path,
            rel: rel.to_string(),
        });
    }

    if entries.is_empty() {
        return None;
    }

    Some(entries)
}

fn should_skip(path: &Path) -> bool {
    for component in path.components() {
        let std::path::Component::Normal(name) = component else {
            continue;
        };
        let Some(name) = name.to_str() else {
            continue;
        };
        if SKIP_DIRS.iter().any(|skip| skip == &name) {
            return true;
        }
    }
    false
}

fn read_text_lossy(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let content = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(err) => {
            let mut text = String::from_utf8_lossy(&err.into_bytes()).into_owned();
            text.retain(|ch| ch != '\u{FFFD}');
            text
        }
    };

    if content.trim().is_empty() {
        return None;
    }

    Some(content)
}

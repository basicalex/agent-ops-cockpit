use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use mermaid_rs_renderer::{
    compute_layout, parse_mermaid, render_svg, write_output_png, Config, RenderConfig, Theme,
};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ThemeMode {
    Auto,
    Dark,
    Light,
}

#[derive(Debug, Parser)]
#[command(
    name = "aoc-yazi-mermaid",
    version,
    about = "Render cached Mermaid previews for Yazi"
)]
struct Args {
    /// Mermaid or Markdown source file.
    #[arg(long)]
    input: PathBuf,

    /// Cache directory for generated previews.
    #[arg(long, env = "AOC_YAZI_MERMAID_CACHE_DIR")]
    cache_dir: Option<PathBuf>,

    /// Preview pane width in terminal cells.
    #[arg(long, default_value_t = 120)]
    cols: u32,

    /// Preview pane height in terminal cells.
    #[arg(long, default_value_t = 40)]
    rows: u32,

    /// Mermaid block index for Markdown sources.
    #[arg(long, default_value_t = 0)]
    block_index: usize,

    /// Preview theme to use.
    #[arg(
        long,
        value_enum,
        env = "AOC_YAZI_MERMAID_THEME",
        default_value = "auto"
    )]
    theme: ThemeMode,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    let input = args
        .input
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", args.input.display()))?;

    let cache_dir = args.cache_dir.clone().unwrap_or_else(default_cache_dir);
    fs::create_dir_all(&cache_dir)
        .with_context(|| format!("failed to create cache dir {}", cache_dir.display()))?;

    let source = load_mermaid_source(&input, args.block_index)?;
    let cache_path = cache_path_for(&cache_dir, &input, &args)?;

    if !is_nonempty_file(&cache_path) {
        render_preview(&source, &cache_path, args.cols, args.rows, args.theme).with_context(
            || {
                format!(
                    "failed to render preview for {} -> {}",
                    input.display(),
                    cache_path.display()
                )
            },
        )?;
    }

    println!("{}", cache_path.display());
    Ok(())
}

fn render_preview(
    source: &str,
    output_path: &Path,
    cols: u32,
    rows: u32,
    theme_mode: ThemeMode,
) -> Result<()> {
    let parsed = parse_mermaid(source).context("invalid Mermaid source")?;

    let mut config = Config::default();
    config.theme = resolve_theme(theme_mode);
    config.render = render_config(cols, rows, &config.theme);

    let layout = compute_layout(&parsed.graph, &config.theme, &config.layout);
    let svg = render_svg(&layout, &config.theme, &config.layout);

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let tmp_path = output_path.with_extension(format!(
        "{}.tmp",
        output_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("png")
    ));

    if tmp_path.exists() {
        let _ = fs::remove_file(&tmp_path);
    }

    write_output_png(&svg, &tmp_path, &config.render, &config.theme)
        .context("PNG encoding failed")?;

    fs::rename(&tmp_path, output_path).with_context(|| {
        format!(
            "failed to move {} to {}",
            tmp_path.display(),
            output_path.display()
        )
    })?;

    Ok(())
}

fn load_mermaid_source(path: &Path, block_index: usize) -> Result<String> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;

    if is_markdown(path) {
        let blocks = extract_mermaid_blocks(&content);
        return blocks.get(block_index).cloned().ok_or_else(|| {
            anyhow::anyhow!(
                "no mermaid block {} found in {}",
                block_index,
                path.display()
            )
        });
    }

    Ok(content)
}

fn cache_path_for(cache_dir: &Path, input: &Path, args: &Args) -> Result<PathBuf> {
    let meta =
        fs::metadata(input).with_context(|| format!("failed to stat {}", input.display()))?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok());

    let mut hasher = Sha256::new();
    hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
    hasher.update(input.to_string_lossy().as_bytes());
    hasher.update(meta.len().to_le_bytes());
    if let Some(modified) = modified {
        hasher.update(modified.as_secs().to_le_bytes());
        hasher.update(modified.subsec_nanos().to_le_bytes());
    }
    hasher.update(args.block_index.to_le_bytes());
    hasher.update(args.cols.to_le_bytes());
    hasher.update(args.rows.to_le_bytes());
    hasher.update([args.theme as u8]);

    let digest = hasher.finalize();
    let mut hex = String::with_capacity(16);
    for byte in &digest[..8] {
        let _ = write!(&mut hex, "{byte:02x}");
    }

    let stem = sanitize_stem(
        input
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("diagram"),
    );

    Ok(cache_dir.join(format!("{stem}-{hex}.png")))
}

fn sanitize_stem(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "diagram".to_string()
    } else {
        trimmed.to_string()
    }
}

fn resolve_theme(mode: ThemeMode) -> Theme {
    match mode {
        ThemeMode::Light => Theme::mermaid_default(),
        ThemeMode::Dark => dark_theme(),
        ThemeMode::Auto => {
            if looks_light_terminal() {
                Theme::mermaid_default()
            } else {
                dark_theme()
            }
        }
    }
}

fn dark_theme() -> Theme {
    let mut theme = Theme::modern();
    theme.primary_color = "#111827".to_string();
    theme.primary_text_color = "#E5E7EB".to_string();
    theme.primary_border_color = "#475569".to_string();
    theme.line_color = "#94A3B8".to_string();
    theme.secondary_color = "#1E293B".to_string();
    theme.tertiary_color = "#0F172A".to_string();
    theme.edge_label_background = "#0F172A".to_string();
    theme.cluster_background = "#111827".to_string();
    theme.cluster_border = "#334155".to_string();
    theme.background = "#0F172A".to_string();
    theme.sequence_actor_fill = "#111827".to_string();
    theme.sequence_actor_border = "#475569".to_string();
    theme.sequence_actor_line = "#64748B".to_string();
    theme.sequence_note_fill = "#1E293B".to_string();
    theme.sequence_note_border = "#475569".to_string();
    theme.sequence_activation_fill = "#1E293B".to_string();
    theme.sequence_activation_border = "#475569".to_string();
    theme.text_color = "#E2E8F0".to_string();
    theme.git_commit_label_color = "#E2E8F0".to_string();
    theme.git_commit_label_background = "#111827".to_string();
    theme.git_tag_label_color = "#E2E8F0".to_string();
    theme.git_tag_label_background = "#1E293B".to_string();
    theme.git_tag_label_border = "#475569".to_string();
    theme.pie_title_text_color = "#E2E8F0".to_string();
    theme.pie_section_text_color = "#E2E8F0".to_string();
    theme.pie_legend_text_color = "#E2E8F0".to_string();
    theme.pie_stroke_color = "#CBD5E1".to_string();
    theme.pie_outer_stroke_color = "#475569".to_string();
    theme
}

fn looks_light_terminal() -> bool {
    if let Ok(mode) = std::env::var("AOC_YAZI_MERMAID_AUTO_THEME") {
        match mode.trim().to_ascii_lowercase().as_str() {
            "light" => return true,
            "dark" => return false,
            _ => {}
        }
    }

    if let Ok(colorfgbg) = std::env::var("COLORFGBG") {
        if let Some(last) = colorfgbg.split(';').next_back() {
            if let Ok(code) = last.trim().parse::<u8>() {
                return matches!(code, 7 | 15);
            }
        }
    }

    false
}

fn render_config(cols: u32, rows: u32, theme: &Theme) -> RenderConfig {
    let ratio = preview_aspect(cols, rows);
    let long_edge = 2200.0_f32;

    let (width, height) = if ratio >= 1.0 {
        let width = long_edge;
        let height = (width / ratio).clamp(900.0, long_edge);
        (width, height)
    } else {
        let height = long_edge;
        let width = (height * ratio).clamp(900.0, long_edge);
        (width, height)
    };

    RenderConfig {
        width,
        height,
        background: theme.background.clone(),
    }
}

fn preview_aspect(cols: u32, rows: u32) -> f32 {
    let cols = cols.max(20) as f32;
    let rows = rows.max(10) as f32;
    (cols / rows).clamp(0.4, 4.0)
}

fn default_cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(dir).join("aoc/yazi-mermaid");
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".cache/aoc/yazi-mermaid");
    }

    PathBuf::from(".cache/aoc/yazi-mermaid")
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let ext = ext.to_ascii_lowercase();
            matches!(ext.as_str(), "md" | "markdown")
        })
        .unwrap_or(false)
}

fn is_nonempty_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|meta| meta.is_file() && meta.len() > 0)
        .unwrap_or(false)
}

fn extract_mermaid_blocks(input: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut in_block = false;
    let mut current = Vec::new();
    let mut fence = String::new();

    for line in input.lines() {
        let trimmed = line.trim();
        if !in_block {
            if let Some(start_fence) = detect_mermaid_fence(trimmed) {
                in_block = true;
                fence = start_fence;
                continue;
            }
        } else if is_fence_end(trimmed, &fence) {
            in_block = false;
            blocks.push(current.join("\n"));
            current.clear();
            continue;
        }

        if in_block {
            current.push(line.to_string());
        }
    }

    blocks
}

fn detect_mermaid_fence(line: &str) -> Option<String> {
    if line.starts_with("```") {
        let rest = line.trim_start_matches('`').trim();
        if rest.starts_with("mermaid") {
            return Some("```".to_string());
        }
    }
    if line.starts_with("~~~") {
        let rest = line.trim_start_matches('~').trim();
        if rest.starts_with("mermaid") {
            return Some("~~~".to_string());
        }
    }
    if line.starts_with(":::") {
        let rest = line.trim_start_matches(':').trim();
        if rest.starts_with("mermaid") {
            return Some(":::".to_string());
        }
    }
    None
}

fn is_fence_end(line: &str, fence: &str) -> bool {
    line.starts_with(fence) && line[fence.len()..].trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_mermaid_blocks_from_markdown() {
        let input = r#"
# Notes

```mermaid
flowchart LR
    A --> B
```

text

~~~mermaid
sequenceDiagram
    Alice->>Bob: hi
~~~
"#;

        let blocks = extract_mermaid_blocks(input);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains("flowchart LR"));
        assert!(blocks[1].contains("sequenceDiagram"));
    }

    #[test]
    fn preview_aspect_is_clamped() {
        assert_eq!(preview_aspect(1, 1), 2.0);
        assert_eq!(preview_aspect(10_000, 1), 4.0);
        assert_eq!(preview_aspect(1, 10_000), 0.4);
    }

    #[test]
    fn stem_is_sanitized() {
        assert_eq!(sanitize_stem("hello world.mmd"), "hello-world-mmd");
    }
}

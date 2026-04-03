use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use clap::{Args, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const MAP_DIR: &str = ".aoc/map";
const PAGES_DIR: &str = ".aoc/map/pages";
const DIAGRAM_FILES_DIR: &str = ".aoc/map/diagrams";
const ASSETS_DIR: &str = ".aoc/map/assets";
const MANIFEST_PATH: &str = ".aoc/map/manifest.json";
const INDEX_PATH: &str = ".aoc/map/index.html";
const README_PATH: &str = ".aoc/map/README.md";
const AOC_MAP_SKILL_PATH: &str = ".pi/skills/aoc-map/SKILL.md";
const LEGACY_AOC_SEE_SKILL_DIR: &str = ".pi/skills/aoc-see";
const MERMAID_JS_PATH: &str = ".aoc/map/assets/mermaid.min.js";
const MERMAID_RENDER_HELPER_PATH: &str = ".aoc/map/assets/render-mermaid.js";
const LEGACY_SEE_DIR: &str = ".aoc/see";
const LEGACY_DIAGRAMS_DIR: &str = ".aoc/diagrams";
const MERMAID_OUTPUT_START: &str = "<!-- aoc-map:mermaid-output:start -->";
const MERMAID_OUTPUT_END: &str = "<!-- aoc-map:mermaid-output:end -->";
const MERMAID_JS_TAG: &str = r#"<script defer src="../assets/mermaid.min.js"></script>"#;
const MERMAID_RENDER_HELPER_TAG: &str =
    r#"<script defer src="../assets/render-mermaid.js"></script>"#;

#[derive(Subcommand, Debug)]
#[command(rename_all = "kebab-case")]
pub enum MapCommand {
    /// Initialize the project-local AOC Map microsite workspace
    Init(InitArgs),
    /// Scaffold a new AOC Map page
    New(NewArgs),
    /// List available AOC Map pages
    List(ListArgs),
    /// Regenerate the AOC Map homepage
    Build(BuildArgs),
    /// Serve the AOC Map microsite over a local dev server
    Serve(ServeArgs),
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Overwrite starter files if they already exist
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct NewArgs {
    /// URL/file slug, e.g. agent-topology
    pub slug: String,
    /// Human-friendly page title
    #[arg(long)]
    pub title: Option<String>,
    /// Optional short summary shown on the homepage
    #[arg(long)]
    pub summary: Option<String>,
    /// Page kind
    #[arg(long, value_enum, default_value_t = DiagramKindArg::Explain)]
    pub kind: DiagramKindArg,
    /// Site collection/section
    #[arg(long, value_enum, default_value_t = DiagramSectionArg::Explainers)]
    pub section: DiagramSectionArg,
    /// Lifecycle status badge shown on the homepage
    #[arg(long, value_enum, default_value_t = DiagramStatusArg::Draft)]
    pub status: DiagramStatusArg,
    /// Comma-delimited tags
    #[arg(long, value_delimiter = ',')]
    pub tags: Vec<String>,
    /// Source paths or references cited by this page
    #[arg(long = "source", value_delimiter = ',')]
    pub source_paths: Vec<String>,
    /// Feature this page near the top of the homepage
    #[arg(long)]
    pub featured: bool,
    /// Mark this page as generated from repo/AOC state
    #[arg(long)]
    pub generated: bool,
    /// Optional manual ordering hint (lower first)
    #[arg(long)]
    pub order: Option<u32>,
    /// Overwrite an existing page/manifest entry
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct BuildArgs {}

#[derive(Args, Debug)]
pub struct ServeArgs {
    /// Bind host
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    /// Bind port
    #[arg(long, default_value_t = 43111)]
    pub port: u16,
    /// Try to open the browser automatically
    #[arg(long)]
    pub open: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum DiagramKindArg {
    Flow,
    Sequence,
    Timeline,
    Topology,
    Dashboard,
    Explain,
    Other,
}

impl DiagramKindArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::Flow => "flow",
            Self::Sequence => "sequence",
            Self::Timeline => "timeline",
            Self::Topology => "topology",
            Self::Dashboard => "dashboard",
            Self::Explain => "explain",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum DiagramSectionArg {
    Architecture,
    Agents,
    Tasks,
    Mind,
    Ops,
    Dashboards,
    Explainers,
    Research,
    Other,
}

impl DiagramSectionArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::Architecture => "architecture",
            Self::Agents => "agents",
            Self::Tasks => "tasks",
            Self::Mind => "mind",
            Self::Ops => "ops",
            Self::Dashboards => "dashboards",
            Self::Explainers => "explainers",
            Self::Research => "research",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum DiagramStatusArg {
    Draft,
    Active,
    Stable,
    Archived,
}

impl DiagramStatusArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Stable => "stable",
            Self::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DiagramManifest {
    #[serde(default = "default_manifest_version")]
    version: u32,
    #[serde(default)]
    site: SiteConfig,
    #[serde(default)]
    diagrams: Vec<DiagramRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SiteConfig {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    project_name: Option<String>,
    #[serde(default)]
    collections: Vec<CollectionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CollectionRecord {
    key: String,
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    order: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DiagramRecord {
    slug: String,
    title: String,
    #[serde(default)]
    page: Option<String>,
    #[serde(default)]
    diagram_path: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    section: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    source_paths: Vec<String>,
    #[serde(default)]
    featured: bool,
    #[serde(default)]
    generated: bool,
    #[serde(default)]
    order: Option<u32>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct DiagramView {
    slug: String,
    title: String,
    page: String,
    diagram_path: Option<String>,
    summary: Option<String>,
    section: String,
    section_title: String,
    kind: String,
    status: String,
    tags: Vec<String>,
    source_paths: Vec<String>,
    featured: bool,
    generated: bool,
    order: Option<u32>,
    created_at: Option<String>,
    updated_at: Option<String>,
    inferred: bool,
}

#[derive(Debug, Clone, Default)]
struct HtmlMetadata {
    title: Option<String>,
    summary: Option<String>,
    section: Option<String>,
    kind: Option<String>,
    status: Option<String>,
    diagram_path: Option<String>,
    tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct SiteView {
    title: String,
    description: String,
    project_name: String,
    project_root: String,
    generated_at: String,
    collections: Vec<CollectionRecord>,
}

#[derive(Debug, Clone)]
struct MapPaths {
    root: PathBuf,
    map_dir: PathBuf,
    pages_dir: PathBuf,
    diagrams_dir: PathBuf,
    assets_dir: PathBuf,
    manifest_path: PathBuf,
    index_path: PathBuf,
    readme_path: PathBuf,
    mermaid_js_path: PathBuf,
    mermaid_render_helper_path: PathBuf,
    legacy_see_dir: PathBuf,
    legacy_diagrams_dir: PathBuf,
}

fn default_manifest_version() -> u32 {
    3
}

pub fn handle_map_command(command: MapCommand) -> Result<()> {
    match command {
        MapCommand::Init(args) => handle_init(args),
        MapCommand::New(args) => handle_new(args),
        MapCommand::List(args) => handle_list(args),
        MapCommand::Build(args) => handle_build(args),
        MapCommand::Serve(args) => handle_serve(args),
    }
}

fn handle_init(args: InitArgs) -> Result<()> {
    let paths = MapPaths::from_root(resolve_project_root()?);
    migrate_legacy_workspace(&paths)?;
    ensure_dirs(&paths)?;
    ensure_aoc_map_skill(&paths.root, args.force)?;

    let mut manifest = load_manifest(&paths.manifest_path)?;
    apply_manifest_defaults(&mut manifest, &paths, args.force);
    write_manifest(&paths.manifest_path, &manifest)?;
    write_if_missing_or_forced(&paths.readme_path, &starter_readme(), args.force)?;
    build_index(&paths)?;

    println!("initialized {}", paths.map_dir.display());
    Ok(())
}

fn handle_new(args: NewArgs) -> Result<()> {
    let paths = MapPaths::from_root(resolve_project_root()?);
    migrate_legacy_workspace(&paths)?;
    ensure_dirs(&paths)?;

    let slug = validate_slug(&args.slug)?;
    let mut manifest = load_manifest(&paths.manifest_path)?;
    apply_manifest_defaults(&mut manifest, &paths, false);

    let title = args.title.unwrap_or_else(|| title_from_slug(&slug));
    let page_name = format!("{slug}.html");
    let page_rel = format!("pages/{page_name}");
    let page_path = paths.pages_dir.join(&page_name);
    let diagram_name = format!("{slug}.mmd");
    let diagram_rel = format!("diagrams/{diagram_name}");
    let diagram_path = paths.diagrams_dir.join(&diagram_name);
    if (page_path.exists() || diagram_path.exists()) && !args.force {
        let existing_path = if page_path.exists() {
            &page_path
        } else {
            &diagram_path
        };
        bail!(
            "{} already exists (use --force to overwrite)",
            existing_path.display()
        );
    }

    let now = Utc::now().to_rfc3339();
    let existing = manifest
        .diagrams
        .iter()
        .find(|entry| entry.slug == slug)
        .cloned();
    let record = DiagramRecord {
        slug: slug.clone(),
        title: title.clone(),
        page: Some(page_rel),
        diagram_path: Some(diagram_rel.clone()),
        summary: args.summary.clone(),
        section: Some(args.section.as_str().to_string()),
        kind: Some(args.kind.as_str().to_string()),
        status: Some(args.status.as_str().to_string()),
        tags: normalize_tokens(args.tags),
        source_paths: normalize_source_paths(args.source_paths),
        featured: args.featured,
        generated: args.generated,
        order: args.order,
        created_at: existing
            .as_ref()
            .and_then(|entry| entry.created_at.clone())
            .or(Some(now.clone())),
        updated_at: Some(now),
    };
    upsert_manifest_record(&mut manifest, record.clone());
    write_manifest(&paths.manifest_path, &manifest)?;

    fs::write(
        &page_path,
        starter_page_html(&manifest.site, &title, &record, &slug),
    )
    .with_context(|| format!("failed to write {}", page_path.display()))?;
    fs::write(&diagram_path, starter_mermaid_source(&title))
        .with_context(|| format!("failed to write {}", diagram_path.display()))?;

    build_index(&paths)?;
    println!("created {}", page_path.display());
    println!("created {}", diagram_path.display());
    Ok(())
}

fn handle_list(args: ListArgs) -> Result<()> {
    let paths = MapPaths::from_root(resolve_project_root()?);
    migrate_legacy_workspace(&paths)?;
    let manifest = load_and_normalize_manifest(&paths)?;
    let diagrams = collect_diagrams(&paths, &manifest)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&diagrams)?);
        return Ok(());
    }
    if diagrams.is_empty() {
        println!("no AOC Map pages found under {}", paths.pages_dir.display());
        return Ok(());
    }

    for diagram in diagrams {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            diagram.slug,
            diagram.section,
            diagram.kind,
            diagram.status,
            diagram.page,
            diagram.summary.unwrap_or_default()
        );
    }
    Ok(())
}

fn handle_build(_args: BuildArgs) -> Result<()> {
    let paths = MapPaths::from_root(resolve_project_root()?);
    migrate_legacy_workspace(&paths)?;
    ensure_dirs(&paths)?;
    build_index(&paths)?;
    println!("built {}", paths.index_path.display());
    Ok(())
}

fn handle_serve(args: ServeArgs) -> Result<()> {
    let paths = MapPaths::from_root(resolve_project_root()?);
    migrate_legacy_workspace(&paths)?;
    ensure_dirs(&paths)?;
    build_index(&paths)?;

    let url = format!("http://{}:{}/", args.host, args.port);
    if args.open {
        let _ = try_open_browser(&url);
    }

    let python = find_python()
        .ok_or_else(|| anyhow!("python3/python is required to serve AOC Map via aoc-map"))?;

    println!("serving {} at {}", paths.map_dir.display(), url);
    println!("press Ctrl-C to stop");

    let status = Command::new(python)
        .args([
            "-m",
            "http.server",
            &args.port.to_string(),
            "--bind",
            &args.host,
            "--directory",
            paths.map_dir.to_string_lossy().as_ref(),
        ])
        .status()
        .context("failed to launch python http.server")?;

    if status.success() {
        Ok(())
    } else {
        bail!("map dev server exited with status {status}")
    }
}

impl MapPaths {
    fn from_root(root: PathBuf) -> Self {
        Self {
            map_dir: root.join(MAP_DIR),
            pages_dir: root.join(PAGES_DIR),
            diagrams_dir: root.join(DIAGRAM_FILES_DIR),
            assets_dir: root.join(ASSETS_DIR),
            manifest_path: root.join(MANIFEST_PATH),
            index_path: root.join(INDEX_PATH),
            readme_path: root.join(README_PATH),
            mermaid_js_path: root.join(MERMAID_JS_PATH),
            mermaid_render_helper_path: root.join(MERMAID_RENDER_HELPER_PATH),
            legacy_see_dir: root.join(LEGACY_SEE_DIR),
            legacy_diagrams_dir: root.join(LEGACY_DIAGRAMS_DIR),
            root,
        }
    }
}

fn resolve_project_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    for candidate in std::iter::once(cwd.as_path()).chain(cwd.ancestors().skip(1)) {
        if candidate.join(".aoc").exists() || candidate.join(".git").exists() {
            return Ok(candidate.to_path_buf());
        }
    }
    Ok(cwd)
}

fn migrate_legacy_workspace(paths: &MapPaths) -> Result<()> {
    if paths.map_dir.exists() {
        return Ok(());
    }

    let legacy_source = if paths.legacy_see_dir.exists() {
        Some(&paths.legacy_see_dir)
    } else if paths.legacy_diagrams_dir.exists() {
        Some(&paths.legacy_diagrams_dir)
    } else {
        None
    };

    let Some(legacy_source) = legacy_source else {
        return Ok(());
    };

    fs::rename(legacy_source, &paths.map_dir).with_context(|| {
        format!(
            "failed to migrate legacy AOC Map workspace from {} to {}",
            legacy_source.display(),
            paths.map_dir.display()
        )
    })?;
    Ok(())
}

fn ensure_dirs(paths: &MapPaths) -> Result<()> {
    fs::create_dir_all(&paths.pages_dir)
        .with_context(|| format!("failed to create {}", paths.pages_dir.display()))?;
    fs::create_dir_all(&paths.diagrams_dir)
        .with_context(|| format!("failed to create {}", paths.diagrams_dir.display()))?;
    fs::create_dir_all(&paths.assets_dir)
        .with_context(|| format!("failed to create {}", paths.assets_dir.display()))?;
    Ok(())
}

fn ensure_aoc_map_skill(root: &Path, force: bool) -> Result<()> {
    let path = root.join(AOC_MAP_SKILL_PATH);
    let target_dir = path
        .parent()
        .ok_or_else(|| anyhow!("invalid AOC Map skill path: {}", path.display()))?;
    let legacy_dir = root.join(LEGACY_AOC_SEE_SKILL_DIR);

    if !target_dir.exists() && legacy_dir.exists() {
        fs::rename(&legacy_dir, target_dir).with_context(|| {
            format!(
                "failed to migrate legacy AOC Map skill from {} to {}",
                legacy_dir.display(),
                target_dir.display()
            )
        })?;
    }

    if path.exists() && !force {
        return Ok(());
    }
    fs::create_dir_all(target_dir)
        .with_context(|| format!("failed to create {}", target_dir.display()))?;
    fs::write(&path, include_str!("../../../.pi/skills/aoc-map/SKILL.md"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn sync_mermaid_assets(paths: &MapPaths) -> Result<()> {
    write_bytes_if_changed(
        &paths.mermaid_js_path,
        include_bytes!("../assets/mermaid.min.js"),
    )?;
    write_string_if_changed(
        &paths.mermaid_render_helper_path,
        include_str!("../assets/render-mermaid.js"),
    )?;
    Ok(())
}

fn normalize_mermaid_pages(paths: &MapPaths) -> Result<()> {
    if !paths.pages_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&paths.pages_dir)
        .with_context(|| format!("failed to read {}", paths.pages_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("html") {
            continue;
        }
        normalize_mermaid_page(&path)?;
    }

    Ok(())
}

fn normalize_mermaid_page(path: &Path) -> Result<()> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let normalized = normalize_mermaid_page_html(&content);
    if normalized != content {
        fs::write(path, normalized)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn normalize_mermaid_page_html(content: &str) -> String {
    let migrated = migrate_legacy_map_markup(&strip_legacy_mermaid_output(content));
    if !migrated.contains("data-aoc-map-mermaid") && !migrated.contains("data-aoc-map-mermaid-src")
    {
        return migrated;
    }
    ensure_mermaid_script_tags(&migrated)
}

fn migrate_legacy_map_markup(content: &str) -> String {
    content
        .replace("data-aoc-see-mermaid-output", "data-aoc-map-mermaid-output")
        .replace("data-aoc-see-mermaid-src", "data-aoc-map-mermaid-src")
        .replace("data-aoc-see-mermaid", "data-aoc-map-mermaid")
        .replace("aoc-see-mermaid-render", "aoc-map-mermaid-render")
        .replace("aoc-see:", "aoc-map:")
        .replace(".aoc/see", ".aoc/map")
        .replace("AOC See", "AOC Map")
        .replace("aoc-see", "aoc-map")
}

fn strip_legacy_mermaid_output(content: &str) -> String {
    let mut output = String::with_capacity(content.len());
    let mut cursor = 0;
    let mut changed = false;

    while let Some(start_rel) = content[cursor..].find(MERMAID_OUTPUT_START) {
        let start = cursor + start_rel;
        output.push_str(&content[cursor..start]);
        let after_start = start + MERMAID_OUTPUT_START.len();
        let Some(end_rel) = content[after_start..].find(MERMAID_OUTPUT_END) else {
            output.push_str(&content[start..]);
            return output;
        };
        let end = after_start + end_rel + MERMAID_OUTPUT_END.len();
        cursor = end;
        changed = true;
    }

    if changed {
        output.push_str(&content[cursor..]);
        output
    } else {
        content.to_string()
    }
}

fn ensure_mermaid_script_tags(content: &str) -> String {
    let needs_mermaid_js = !content.contains("mermaid.min.js");
    let needs_render_helper = !content.contains("render-mermaid.js");
    if !needs_mermaid_js && !needs_render_helper {
        return content.to_string();
    }

    let mut injection = String::new();
    if needs_mermaid_js {
        injection.push_str("\n  ");
        injection.push_str(MERMAID_JS_TAG);
    }
    if needs_render_helper {
        injection.push_str("\n  ");
        injection.push_str(MERMAID_RENDER_HELPER_TAG);
    }

    if let Some(index) = content.find("</head>") {
        let mut output = String::with_capacity(content.len() + injection.len() + 1);
        output.push_str(&content[..index]);
        output.push_str(&injection);
        output.push('\n');
        output.push_str(&content[index..]);
        return output;
    }

    if let Some(index) = content.find("</body>") {
        let mut output = String::with_capacity(content.len() + injection.len() + 1);
        output.push_str(&content[..index]);
        output.push_str(&injection);
        output.push('\n');
        output.push_str(&content[index..]);
        return output;
    }

    format!("{content}{injection}\n")
}

fn write_bytes_if_changed(path: &Path, bytes: &[u8]) -> Result<()> {
    let should_write = match fs::read(path) {
        Ok(existing) => existing != bytes,
        Err(_) => true,
    };
    if should_write {
        fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn write_string_if_changed(path: &Path, content: &str) -> Result<()> {
    let should_write = match fs::read_to_string(path) {
        Ok(existing) => existing != content,
        Err(_) => true,
    };
    if should_write {
        fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn write_if_missing_or_forced(path: &Path, content: &str, force: bool) -> Result<()> {
    if path.exists() && !force {
        return Ok(());
    }
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))
}

fn load_manifest(path: &Path) -> Result<DiagramManifest> {
    if !path.exists() {
        return Ok(DiagramManifest {
            version: default_manifest_version(),
            site: SiteConfig::default(),
            diagrams: Vec::new(),
        });
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: DiagramManifest = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(manifest)
}

fn load_and_normalize_manifest(paths: &MapPaths) -> Result<DiagramManifest> {
    let mut manifest = load_manifest(&paths.manifest_path)?;
    apply_manifest_defaults(&mut manifest, paths, false);
    if !paths.manifest_path.exists() || manifest.version != default_manifest_version() {
        manifest.version = default_manifest_version();
        write_manifest(&paths.manifest_path, &manifest)?;
    }
    Ok(manifest)
}

fn write_manifest(path: &Path, manifest: &DiagramManifest) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(manifest)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn apply_manifest_defaults(manifest: &mut DiagramManifest, paths: &MapPaths, force: bool) {
    manifest.version = default_manifest_version();

    let defaults = default_site_config(paths);
    if force
        || manifest
            .site
            .title
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        manifest.site.title = defaults.title;
    }
    if force
        || manifest
            .site
            .description
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        manifest.site.description = defaults.description;
    }
    if force
        || manifest
            .site
            .project_name
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        manifest.site.project_name = defaults.project_name;
    }
    if force || manifest.site.collections.is_empty() {
        manifest.site.collections = defaults.collections;
    } else {
        manifest.site.collections = normalized_collections(manifest.site.collections.clone());
    }

    for record in &mut manifest.diagrams {
        if force
            || record
                .diagram_path
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            record.diagram_path = Some(format!("diagrams/{}.mmd", record.slug));
        }
    }
}

fn default_site_config(paths: &MapPaths) -> SiteConfig {
    let repo_name = paths
        .root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project")
        .to_string();
    let pretty = title_from_slug(&repo_name.replace(['_', ' '], "-"));
    SiteConfig {
        title: Some(format!("{} · AOC Map", pretty)),
        description: Some(format!(
            "Graph-first visualization layer for {}. Use this microsite to browse project flows, architecture maps, agent routes, task views, and other minimal visual explanations.",
            pretty
        )),
        project_name: Some(pretty),
        collections: default_collections(),
    }
}

fn normalized_collections(mut collections: Vec<CollectionRecord>) -> Vec<CollectionRecord> {
    if collections.is_empty() {
        return default_collections();
    }
    for collection in &mut collections {
        collection.key = normalize_key(&collection.key);
        if collection.title.trim().is_empty() {
            collection.title = title_from_slug(&collection.key);
        }
    }
    collections.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.title.cmp(&right.title))
    });
    collections
}

fn default_collections() -> Vec<CollectionRecord> {
    vec![
        CollectionRecord {
            key: "architecture".to_string(),
            title: "Architecture".to_string(),
            description: Some(
                "System structure, boundaries, core data flow, and major implementation surfaces."
                    .to_string(),
            ),
            order: 10,
        },
        CollectionRecord {
            key: "agents".to_string(),
            title: "Agents".to_string(),
            description: Some(
                "Specialist roles, subagents, orchestration routes, wrappers, and runtime behavior."
                    .to_string(),
            ),
            order: 20,
        },
        CollectionRecord {
            key: "tasks".to_string(),
            title: "Tasks".to_string(),
            description: Some(
                "Taskmaster flows, dependencies, delivery plans, and work decomposition."
                    .to_string(),
            ),
            order: 30,
        },
        CollectionRecord {
            key: "mind".to_string(),
            title: "Mind".to_string(),
            description: Some(
                "Insight, provenance, compaction, observer/reflector paths, and memory views."
                    .to_string(),
            ),
            order: 40,
        },
        CollectionRecord {
            key: "ops".to_string(),
            title: "Ops".to_string(),
            description: Some(
                "Operational runbooks, session lifecycle pages, troubleshooting surfaces, and control flows."
                    .to_string(),
            ),
            order: 50,
        },
        CollectionRecord {
            key: "dashboards".to_string(),
            title: "Dashboards".to_string(),
            description: Some(
                "Visual summaries, fleet views, status boards, and rollups derived from repo state."
                    .to_string(),
            ),
            order: 60,
        },
        CollectionRecord {
            key: "explainers".to_string(),
            title: "Explainers".to_string(),
            description: Some(
                "Narrative visual walkthroughs for humans and agents who need fast orientation."
                    .to_string(),
            ),
            order: 70,
        },
        CollectionRecord {
            key: "research".to_string(),
            title: "Research".to_string(),
            description: Some(
                "Investigations, comparisons, spikes, and references captured as visual notes."
                    .to_string(),
            ),
            order: 80,
        },
        CollectionRecord {
            key: "other".to_string(),
            title: "Other".to_string(),
            description: Some("Everything else that benefits from a browsable visual artifact.".to_string()),
            order: 90,
        },
    ]
}

fn validate_slug(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("slug cannot be empty");
    }
    let valid = trimmed
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-');
    if !valid || trimmed.starts_with('-') || trimmed.ends_with('-') {
        bail!("slug must match [a-z0-9-] and cannot start/end with '-'");
    }
    Ok(trimmed.to_string())
}

fn title_from_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_key(value: &str) -> String {
    let mut normalized = String::new();
    let mut last_dash = false;
    for ch in value.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else if matches!(ch, '-' | '_' | ' ') {
            '-'
        } else {
            continue;
        };
        if mapped == '-' {
            if last_dash || normalized.is_empty() {
                continue;
            }
            last_dash = true;
            normalized.push(mapped);
        } else {
            last_dash = false;
            normalized.push(mapped);
        }
    }
    normalized.trim_matches('-').to_string()
}

fn normalize_tokens(tokens: Vec<String>) -> Vec<String> {
    let mut deduped = BTreeSet::new();
    for token in tokens {
        let normalized = normalize_key(&token);
        if !normalized.is_empty() {
            deduped.insert(normalized);
        }
    }
    deduped.into_iter().collect()
}

fn normalize_source_paths(values: Vec<String>) -> Vec<String> {
    let mut deduped = BTreeSet::new();
    for value in values {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            deduped.insert(trimmed.to_string());
        }
    }
    deduped.into_iter().collect()
}

fn upsert_manifest_record(manifest: &mut DiagramManifest, record: DiagramRecord) {
    if let Some(existing) = manifest
        .diagrams
        .iter_mut()
        .find(|entry| entry.slug == record.slug)
    {
        *existing = record;
    } else {
        manifest.diagrams.push(record);
    }
    manifest.diagrams.sort_by(|left, right| {
        left.order
            .unwrap_or(u32::MAX)
            .cmp(&right.order.unwrap_or(u32::MAX))
            .then_with(|| left.slug.cmp(&right.slug))
    });
}

fn collect_diagrams(paths: &MapPaths, manifest: &DiagramManifest) -> Result<Vec<DiagramView>> {
    let by_slug: BTreeMap<_, _> = manifest
        .diagrams
        .iter()
        .cloned()
        .map(|record| (record.slug.clone(), record))
        .collect();
    let by_page: BTreeMap<_, _> = manifest
        .diagrams
        .iter()
        .cloned()
        .filter_map(|record| record.page.clone().map(|page| (page, record)))
        .collect();

    let mut views = Vec::new();
    if paths.pages_dir.exists() {
        for entry in fs::read_dir(&paths.pages_dir)
            .with_context(|| format!("failed to read {}", paths.pages_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("html") {
                continue;
            }

            let page_rel = format!(
                "pages/{}",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| anyhow!("invalid page filename {}", path.display()))?
            );
            let slug = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| anyhow!("invalid page filename {}", path.display()))?
                .to_string();
            let html_meta = extract_html_metadata(&path)?;
            let record = by_page.get(&page_rel).or_else(|| by_slug.get(&slug));
            let section = record
                .and_then(|item| item.section.clone())
                .or(html_meta.section)
                .map(|value| normalize_key(&value))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| infer_section(record.and_then(|item| item.kind.as_deref())));
            let kind = record
                .and_then(|item| item.kind.clone())
                .or(html_meta.kind)
                .map(|value| normalize_key(&value))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "other".to_string());
            let status = record
                .and_then(|item| item.status.clone())
                .or(html_meta.status)
                .map(|value| normalize_key(&value))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "draft".to_string());
            let mut tags = record.map(|item| item.tags.clone()).unwrap_or_default();
            tags.extend(html_meta.tags);
            let tags = normalize_tokens(tags);

            views.push(DiagramView {
                slug: slug.clone(),
                title: record
                    .map(|item| item.title.clone())
                    .filter(|value| !value.trim().is_empty())
                    .or(html_meta.title)
                    .unwrap_or_else(|| title_from_slug(&slug)),
                page: page_rel,
                diagram_path: record
                    .and_then(|item| item.diagram_path.clone())
                    .or(html_meta.diagram_path),
                summary: record
                    .and_then(|item| item.summary.clone())
                    .or(html_meta.summary),
                section_title: section_title(&section, &manifest.site.collections),
                section,
                kind,
                status,
                tags,
                source_paths: record
                    .map(|item| item.source_paths.clone())
                    .unwrap_or_default(),
                featured: record.map(|item| item.featured).unwrap_or(false),
                generated: record.map(|item| item.generated).unwrap_or(false),
                order: record.and_then(|item| item.order),
                created_at: record.and_then(|item| item.created_at.clone()),
                updated_at: record.and_then(|item| item.updated_at.clone()),
                inferred: record.is_none(),
            });
        }
    }

    views.sort_by(|left, right| compare_diagrams(left, right));
    Ok(views)
}

fn compare_diagrams(left: &DiagramView, right: &DiagramView) -> std::cmp::Ordering {
    right
        .featured
        .cmp(&left.featured)
        .then_with(|| {
            left.order
                .unwrap_or(u32::MAX)
                .cmp(&right.order.unwrap_or(u32::MAX))
        })
        .then_with(|| right.updated_at.cmp(&left.updated_at))
        .then_with(|| left.title.cmp(&right.title))
}

fn infer_section(kind: Option<&str>) -> String {
    match kind.map(normalize_key).as_deref() {
        Some("dashboard") => "dashboards".to_string(),
        Some("topology") => "agents".to_string(),
        Some("timeline") => "ops".to_string(),
        _ => "explainers".to_string(),
    }
}

fn extract_html_metadata(path: &Path) -> Result<HtmlMetadata> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(HtmlMetadata {
        title: extract_tag_text(&content, "title").or_else(|| extract_tag_text(&content, "h1")),
        summary: extract_named_meta_compat(&content, "summary"),
        section: extract_named_meta_compat(&content, "section"),
        kind: extract_named_meta_compat(&content, "kind"),
        status: extract_named_meta_compat(&content, "status"),
        diagram_path: extract_named_meta_compat(&content, "diagram"),
        tags: extract_named_meta_compat(&content, "tags")
            .map(|value| {
                value
                    .split(',')
                    .map(|part| part.trim().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    })
}

fn extract_named_meta_compat(content: &str, suffix: &str) -> Option<String> {
    extract_named_meta(content, &format!("aoc-map:{suffix}"))
        .or_else(|| extract_named_meta(content, &format!("aoc-see:{suffix}")))
}

fn extract_named_meta(content: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("name={quote}{name}{quote}");
        let Some(start) = content.find(&needle) else {
            continue;
        };
        let rest = &content[start..];
        let Some(tag_end) = rest.find('>') else {
            continue;
        };
        let tag = &rest[..tag_end];
        if let Some(value) = extract_attr(tag, "content") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{attr}={quote}");
        if let Some(start) = tag.find(&needle) {
            let tail = &tag[start + needle.len()..];
            let end = tail.find(quote)?;
            return Some(tail[..end].to_string());
        }
    }
    None
}

fn extract_tag_text(content: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = content.find(&open)?;
    let rest = &content[start..];
    let open_end = rest.find('>')? + 1;
    let close = format!("</{tag}>");
    let inner = &rest[open_end..];
    let end = inner.find(&close)?;
    let value = inner[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn build_index(paths: &MapPaths) -> Result<()> {
    sync_mermaid_assets(paths)?;
    normalize_mermaid_pages(paths)?;
    let manifest = load_and_normalize_manifest(paths)?;
    let diagrams = collect_diagrams(paths, &manifest)?;
    let site = build_site_view(paths, &manifest);
    let html = render_index_html(&site, &diagrams);
    fs::write(&paths.index_path, html)
        .with_context(|| format!("failed to write {}", paths.index_path.display()))?;
    Ok(())
}

fn build_site_view(paths: &MapPaths, manifest: &DiagramManifest) -> SiteView {
    let defaults = default_site_config(paths);
    SiteView {
        title: manifest
            .site
            .title
            .clone()
            .or(defaults.title)
            .unwrap_or_else(|| "AOC Map".to_string()),
        description: manifest
            .site
            .description
            .clone()
            .or(defaults.description)
            .unwrap_or_else(|| "Project-local visualization layer".to_string()),
        project_name: manifest
            .site
            .project_name
            .clone()
            .or(defaults.project_name)
            .unwrap_or_else(|| "Project".to_string()),
        project_root: paths.root.display().to_string(),
        generated_at: Utc::now().to_rfc3339(),
        collections: normalized_collections(manifest.site.collections.clone()),
    }
}

fn render_index_html(site: &SiteView, diagrams: &[DiagramView]) -> String {
    let total_pages = diagrams.len();
    let section_keys = used_section_keys(diagrams, &site.collections);
    let unique_tags = collect_unique_tags(diagrams);
    let unique_kinds = collect_unique_kinds(diagrams);
    let visible_section_count = section_keys.len();
    let recent = recent_diagrams(diagrams);
    let section_nav = render_section_nav(&section_keys, &site.collections);
    let section_blocks = render_sections(diagrams, &section_keys, &site.collections);
    let featured_block = render_featured(diagrams);
    let recent_block = render_recent(&recent);
    let section_filters = render_filter_buttons("section", &section_keys);
    let kind_filters = render_filter_buttons("kind", &unique_kinds);
    let tag_filters = render_filter_buttons("tag", &unique_tags);
    let empty_state = if total_pages == 0 {
        "<section class=\"empty-site\"><h2>No pages yet</h2><p>This repo already has an AOC Map homepage. Now start adding pages with <code>aoc-map new agent-topology --section agents --kind topology</code>, then rebuild or serve the site.</p></section>".to_string()
    } else {
        String::new()
    };

    format!(
        r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <meta name="description" content="{description}">
  <style>
    :root {{
      color-scheme: dark;
      --bg: #07111d;
      --bg-2: #0b1727;
      --panel: rgba(12, 20, 36, 0.9);
      --panel-2: rgba(15, 26, 46, 0.95);
      --line: #243756;
      --line-soft: rgba(120, 158, 220, 0.14);
      --text: #eaf1ff;
      --muted: #9eb1d1;
      --accent: #79c0ff;
      --accent-2: #8dd694;
      --accent-3: #f2cc8f;
      --danger: #ef8d8d;
      --shadow: 0 20px 40px rgba(0, 0, 0, 0.28);
      --radius: 18px;
    }}
    * {{ box-sizing: border-box; }}
    html {{ scroll-behavior: smooth; }}
    body {{
      margin: 0;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      background:
        radial-gradient(circle at top right, rgba(121, 192, 255, 0.14), transparent 25%),
        radial-gradient(circle at top left, rgba(141, 214, 148, 0.10), transparent 25%),
        linear-gradient(180deg, var(--bg), #091321 45%, #08111c);
      color: var(--text);
    }}
    a {{ color: var(--accent); text-decoration: none; }}
    a:hover {{ text-decoration: underline; }}
    code {{
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: 0.92em;
      color: var(--accent);
      background: rgba(8, 16, 28, 0.85);
      border: 1px solid var(--line);
      border-radius: 8px;
      padding: 0.15rem 0.38rem;
    }}
    .shell {{ max-width: 1260px; margin: 0 auto; padding: 24px 20px 72px; }}
    .topbar {{
      position: sticky; top: 0; z-index: 20; backdrop-filter: blur(14px);
      background: rgba(7, 12, 20, 0.72); border: 1px solid var(--line-soft);
      border-radius: 16px; padding: 12px 16px; display: flex; gap: 12px; justify-content: space-between; align-items: center;
      margin-bottom: 18px;
    }}
    .brand {{ display: flex; gap: 12px; align-items: center; min-width: 0; }}
    .brand-badge {{
      display: inline-flex; align-items: center; justify-content: center; width: 42px; height: 42px;
      border-radius: 12px; background: linear-gradient(135deg, rgba(121,192,255,0.25), rgba(141,214,148,0.18));
      border: 1px solid var(--line); font-weight: 800;
    }}
    .brand-title {{ font-weight: 700; font-size: 1rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }}
    .brand-subtitle {{ color: var(--muted); font-size: 0.88rem; }}
    .nav-links {{ display: flex; flex-wrap: wrap; gap: 8px; justify-content: flex-end; }}
    .nav-links a {{
      display: inline-flex; align-items: center; gap: 8px; padding: 8px 12px; border-radius: 999px;
      border: 1px solid var(--line); background: rgba(12, 20, 36, 0.7); color: var(--muted);
    }}
    .hero {{
      display: grid; grid-template-columns: minmax(0, 1.7fr) minmax(320px, 0.9fr);
      gap: 18px; margin: 24px 0 18px;
    }}
    .panel {{
      background: linear-gradient(180deg, rgba(17,27,45,0.9), rgba(10,17,31,0.92));
      border: 1px solid var(--line); border-radius: var(--radius); box-shadow: var(--shadow);
    }}
    .hero-main {{ padding: 28px; }}
    .eyebrow {{
      display: inline-flex; align-items: center; gap: 8px; padding: 7px 12px; margin-bottom: 18px;
      border-radius: 999px; border: 1px solid var(--line); color: var(--accent); background: rgba(10, 18, 31, 0.88);
      font-size: 0.88rem; font-weight: 600; letter-spacing: 0.04em; text-transform: uppercase;
    }}
    h1 {{ margin: 0 0 12px; font-size: clamp(2rem, 5vw, 3.2rem); line-height: 1.04; }}
    .hero p {{ margin: 0; color: var(--muted); line-height: 1.65; }}
    .hero-actions {{ display: flex; flex-wrap: wrap; gap: 10px; margin-top: 20px; }}
    .button {{
      display: inline-flex; align-items: center; gap: 10px; padding: 11px 14px; border-radius: 12px;
      border: 1px solid var(--line); text-decoration: none; color: var(--text); background: rgba(12, 20, 36, 0.86);
      font-weight: 600;
    }}
    .button.primary {{ color: #05101c; background: linear-gradient(135deg, var(--accent), #9fd2ff); border-color: transparent; }}
    .hero-side {{ padding: 22px; display: grid; gap: 14px; align-content: start; }}
    .mini-grid {{ display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }}
    .metric {{ padding: 15px; border-radius: 16px; background: rgba(8, 15, 26, 0.82); border: 1px solid var(--line); }}
    .metric-label {{ color: var(--muted); font-size: 0.86rem; margin-bottom: 6px; }}
    .metric-value {{ font-size: 1.45rem; font-weight: 800; }}
    .note {{ padding: 14px 15px; border-radius: 14px; border: 1px solid var(--line); background: rgba(8, 15, 26, 0.82); }}
    .note strong {{ display: block; margin-bottom: 6px; }}
    .search-panel {{ padding: 18px; margin: 18px 0; }}
    .search-row {{ display: flex; flex-wrap: wrap; gap: 12px; align-items: center; justify-content: space-between; margin-bottom: 16px; }}
    .search-row h2 {{ margin: 0; font-size: 1.08rem; }}
    .search-row p {{ margin: 0; color: var(--muted); }}
    .search-box {{ width: 100%; }}
    .search-box input {{
      width: 100%; border: 1px solid var(--line); border-radius: 14px; background: rgba(8, 14, 24, 0.92);
      color: var(--text); padding: 13px 14px; font-size: 1rem; outline: none;
    }}
    .search-box input:focus {{ border-color: var(--accent); box-shadow: 0 0 0 3px rgba(121, 192, 255, 0.12); }}
    .filter-stack {{ display: grid; gap: 14px; }}
    .filter-group-label {{ color: var(--muted); font-size: 0.86rem; margin-bottom: 8px; text-transform: uppercase; letter-spacing: 0.06em; }}
    .chips {{ display: flex; flex-wrap: wrap; gap: 8px; }}
    .chip {{
      border: 1px solid var(--line); background: rgba(12, 20, 36, 0.82); color: var(--muted);
      border-radius: 999px; padding: 8px 12px; cursor: pointer; font: inherit;
    }}
    .chip.active {{ background: rgba(121, 192, 255, 0.14); color: var(--text); border-color: rgba(121, 192, 255, 0.5); }}
    .layout {{ display: grid; grid-template-columns: minmax(0, 1.65fr) minmax(300px, 0.75fr); gap: 18px; align-items: start; }}
    .stack {{ display: grid; gap: 18px; }}
    .featured {{ padding: 20px; }}
    .featured h2, .section-header h2, .sidebar-card h2 {{ margin: 0 0 8px; font-size: 1.12rem; }}
    .section-header p, .featured p, .sidebar-card p {{ margin: 0; color: var(--muted); line-height: 1.55; }}
    .card-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 14px; margin-top: 16px; }}
    .card {{
      display: grid; gap: 12px; align-content: start; padding: 18px;
      border-radius: 18px; background: linear-gradient(180deg, rgba(12, 20, 36, 0.95), rgba(9, 16, 28, 0.98));
      border: 1px solid var(--line); min-height: 255px;
    }}
    .card-top {{ display: flex; gap: 8px; flex-wrap: wrap; align-items: center; justify-content: space-between; }}
    .badge-row {{ display: flex; gap: 8px; flex-wrap: wrap; }}
    .badge {{
      display: inline-flex; align-items: center; gap: 6px; padding: 5px 10px; border-radius: 999px;
      border: 1px solid var(--line); font-size: 0.8rem; background: rgba(10, 17, 29, 0.9); color: var(--muted);
      text-transform: uppercase; letter-spacing: 0.04em;
    }}
    .badge.kind {{ color: var(--accent); }}
    .badge.status-stable {{ color: var(--accent-2); }}
    .badge.status-active {{ color: var(--accent); }}
    .badge.status-draft {{ color: var(--accent-3); }}
    .badge.status-archived {{ color: var(--danger); }}
    .card h3 {{ margin: 0; font-size: 1.1rem; line-height: 1.3; }}
    .card p {{ margin: 0; color: var(--muted); line-height: 1.55; }}
    .meta-list {{ display: grid; gap: 8px; color: var(--muted); font-size: 0.92rem; }}
    .tags {{ display: flex; flex-wrap: wrap; gap: 8px; }}
    .tag {{ border-radius: 999px; background: rgba(21, 34, 57, 0.92); border: 1px solid var(--line); color: var(--muted); padding: 5px 10px; font-size: 0.85rem; }}
    .section-block {{ padding: 20px; }}
    .section-header {{ display: flex; gap: 12px; justify-content: space-between; align-items: start; margin-bottom: 6px; }}
    .section-count {{ color: var(--muted); font-size: 0.92rem; white-space: nowrap; }}
    .sidebar {{ display: grid; gap: 18px; }}
    .sidebar-card {{ padding: 18px; }}
    .list {{ display: grid; gap: 10px; margin-top: 14px; }}
    .list-item {{ padding: 12px 13px; border-radius: 14px; border: 1px solid var(--line); background: rgba(8, 14, 24, 0.88); }}
    .list-item strong {{ display: block; margin-bottom: 5px; }}
    .section-block.hidden, .card.hidden, #no-results.hidden, #empty-sections.hidden {{ display: none; }}
    .empty-site, .empty-results {{ padding: 20px; border-radius: 18px; border: 1px dashed var(--line); background: rgba(8, 14, 24, 0.78); color: var(--muted); }}
    footer {{ margin-top: 24px; padding: 18px; color: var(--muted); border: 1px solid var(--line); border-radius: 16px; background: rgba(8, 14, 24, 0.82); }}
    @media (max-width: 980px) {{
      .hero, .layout {{ grid-template-columns: 1fr; }}
      .topbar {{ position: static; }}
    }}
  </style>
</head>
<body>
  <div class="shell">
    <div class="topbar">
      <div class="brand">
        <div class="brand-badge">AØ</div>
        <div>
          <div class="brand-title">{title}</div>
          <div class="brand-subtitle">{project_name} · project-local visualization microsite</div>
        </div>
      </div>
      <nav class="nav-links">
        <a href="#filters">Filters</a>
        <a href="#collections">Collections</a>
        <a href="#recent">Recent</a>
      </nav>
    </div>

    <section class="hero">
      <div class="panel hero-main">
        <div class="eyebrow">AOC Map · Visualization Layer</div>
        <h1>Explore the repo as a browsable site.</h1>
        <p>{description}</p>
        <div class="hero-actions">
          <a class="button primary" href="#collections">Browse pages</a>
          <a class="button" href="#filters">Search & filter</a>
          <span class="button"><code>aoc-map serve --open</code></span>
        </div>
      </div>
      <aside class="panel hero-side">
        <div class="mini-grid">
          <div class="metric">
            <div class="metric-label">Pages</div>
            <div class="metric-value" id="visible-page-count">{total_pages}</div>
          </div>
          <div class="metric">
            <div class="metric-label">Collections</div>
            <div class="metric-value">{visible_section_count}</div>
          </div>
          <div class="metric">
            <div class="metric-label">Kinds</div>
            <div class="metric-value">{kind_count}</div>
          </div>
          <div class="metric">
            <div class="metric-label">Tags</div>
            <div class="metric-value">{tag_count}</div>
          </div>
        </div>
        <div class="note">
          <strong>How to add a page</strong>
          Scaffold with <code>aoc-map new task-flow --section tasks --kind flow</code>, then edit the generated Mermaid in <code>.aoc/map/diagrams/</code> and keep the page in <code>.aoc/map/pages/</code> graph-first.
        </div>
        <div class="note">
          <strong>Project root</strong>
          <code>{project_root}</code>
        </div>
      </aside>
    </section>

    <section class="panel search-panel" id="filters">
      <div class="search-row">
        <div>
          <h2>Discover pages fast</h2>
          <p>Filter by collection, page kind, tag, or free-text search.</p>
        </div>
        <div class="badge-row">
          <span class="badge">generated {generated_at}</span>
          <span class="badge">{total_pages} total pages</span>
        </div>
      </div>
      <div class="search-box">
        <input id="search-input" type="search" placeholder="Search by title, summary, tag, section, or kind..." autocomplete="off">
      </div>
      <div class="filter-stack">
        <div>
          <div class="filter-group-label">Collections</div>
          <div class="chips" data-group="section">
            <button class="chip active" type="button" data-filter-group="section" data-filter-value="all">All</button>
            {section_filters}
          </div>
        </div>
        <div>
          <div class="filter-group-label">Kinds</div>
          <div class="chips" data-group="kind">
            <button class="chip active" type="button" data-filter-group="kind" data-filter-value="all">All</button>
            {kind_filters}
          </div>
        </div>
        <div>
          <div class="filter-group-label">Tags</div>
          <div class="chips" data-group="tag">
            <button class="chip active" type="button" data-filter-group="tag" data-filter-value="all">All</button>
            {tag_filters}
          </div>
        </div>
      </div>
    </section>

    {empty_state}

    <div class="layout">
      <main class="stack" id="collections">
        {featured_block}
        {section_nav}
        <div id="no-results" class="empty-results hidden">
          <h2>No matching pages</h2>
          <p>Try clearing some filters or search terms.</p>
        </div>
        <div id="empty-sections" class="hidden"></div>
        {section_blocks}
      </main>
      <aside class="sidebar">
        <section class="panel sidebar-card" id="recent">
          <h2>Recent updates</h2>
          <p>Most recently updated pages in this microsite.</p>
          {recent_block}
        </section>
        <section class="panel sidebar-card">
          <h2>Suggested collections</h2>
          <p>Good homepage categories for long-lived AOC projects.</p>
          <div class="list">
            <div class="list-item"><strong>Architecture</strong>Boundaries, components, data flow, and repo layout.</div>
            <div class="list-item"><strong>Agents</strong>Roles, subagent orchestration, wrappers, sessions, and handoffs.</div>
            <div class="list-item"><strong>Tasks & Mind</strong>Task graphs, provenance, compaction, exports, and recovery paths.</div>
            <div class="list-item"><strong>Ops</strong>Runbooks, debugging surfaces, status boards, and lifecycle pages.</div>
          </div>
        </section>
      </aside>
    </div>

    <footer>
      <strong>{title}</strong> is generated from <code>.aoc/map/manifest.json</code> plus any graph-backed pages in <code>.aoc/map/pages/</code>. Mermaid source files live in <code>.aoc/map/diagrams/</code>. Build with <code>aoc-map build</code> and serve with <code>aoc-map serve</code>.
    </footer>
  </div>

  <script>
    (() => {{
      const state = {{ section: 'all', kind: 'all', tag: 'all', query: '' }};
      const cards = Array.from(document.querySelectorAll('.card[data-card="1"]'));
      const sections = Array.from(document.querySelectorAll('.section-block[data-section-block]'));
      const visibleCount = document.getElementById('visible-page-count');
      const noResults = document.getElementById('no-results');
      const searchInput = document.getElementById('search-input');

      function normalize(value) {{
        return (value || '').toLowerCase();
      }}

      function matches(card) {{
        const query = normalize(state.query);
        const haystack = normalize(card.dataset.search || '');
        if (query && !haystack.includes(query)) return false;
        if (state.section !== 'all' && normalize(card.dataset.section) !== state.section) return false;
        if (state.kind !== 'all' && normalize(card.dataset.kind) !== state.kind) return false;
        if (state.tag !== 'all') {{
          const tags = normalize(card.dataset.tags || '').split(' ').filter(Boolean);
          if (!tags.includes(state.tag)) return false;
        }}
        return true;
      }}

      function apply() {{
        let visible = 0;
        cards.forEach(card => {{
          const show = matches(card);
          card.classList.toggle('hidden', !show);
          if (show) visible += 1;
        }});

        sections.forEach(section => {{
          const cardsInSection = Array.from(section.querySelectorAll('.card[data-card="1"]'));
          const anyVisible = cardsInSection.some(card => !card.classList.contains('hidden'));
          section.classList.toggle('hidden', !anyVisible);
          const counter = section.querySelector('[data-section-count]');
          if (counter) counter.textContent = `${{cardsInSection.filter(card => !card.classList.contains('hidden')).length}} pages`;
        }});

        visibleCount.textContent = String(visible);
        noResults.classList.toggle('hidden', visible !== 0);
      }}

      document.querySelectorAll('.chip[data-filter-group]').forEach(button => {{
        button.addEventListener('click', () => {{
          const group = button.dataset.filterGroup;
          const value = normalize(button.dataset.filterValue || 'all');
          state[group] = value;
          document.querySelectorAll(`.chip[data-filter-group="${{group}}"]`).forEach(el => el.classList.remove('active'));
          button.classList.add('active');
          apply();
        }});
      }});

      if (searchInput) {{
        searchInput.addEventListener('input', event => {{
          state.query = event.target.value || '';
          apply();
        }});
      }}

      apply();
    }})();
  </script>
</body>
</html>
"##,
        title = escape_html(&site.title),
        description = escape_html(&site.description),
        project_name = escape_html(&site.project_name),
        project_root = escape_html(&site.project_root),
        generated_at = escape_html(&site.generated_at),
        total_pages = total_pages,
        visible_section_count = visible_section_count,
        kind_count = unique_kinds.len(),
        tag_count = unique_tags.len(),
        section_filters = section_filters,
        kind_filters = kind_filters,
        tag_filters = tag_filters,
        featured_block = featured_block,
        section_nav = section_nav,
        section_blocks = section_blocks,
        recent_block = recent_block,
        empty_state = empty_state,
    )
}

fn used_section_keys(diagrams: &[DiagramView], collections: &[CollectionRecord]) -> Vec<String> {
    let mut used: BTreeSet<String> = diagrams
        .iter()
        .map(|diagram| diagram.section.clone())
        .collect();
    if used.is_empty() {
        used = collections.iter().map(|item| item.key.clone()).collect();
    }

    let mut ordered = Vec::new();
    for collection in collections {
        if used.remove(&collection.key) {
            ordered.push(collection.key.clone());
        }
    }
    let mut rest: Vec<_> = used.into_iter().collect();
    rest.sort();
    ordered.extend(rest);
    ordered
}

fn collect_unique_tags(diagrams: &[DiagramView]) -> Vec<String> {
    let mut values = BTreeSet::new();
    for diagram in diagrams {
        for tag in &diagram.tags {
            values.insert(tag.clone());
        }
    }
    values.into_iter().collect()
}

fn collect_unique_kinds(diagrams: &[DiagramView]) -> Vec<String> {
    let mut values = BTreeSet::new();
    for diagram in diagrams {
        values.insert(diagram.kind.clone());
    }
    values.into_iter().collect()
}

fn recent_diagrams(diagrams: &[DiagramView]) -> Vec<DiagramView> {
    let mut items = diagrams.to_vec();
    items.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.title.cmp(&right.title))
    });
    items.into_iter().take(6).collect()
}

fn render_section_nav(section_keys: &[String], collections: &[CollectionRecord]) -> String {
    if section_keys.is_empty() {
        return String::new();
    }
    let links = section_keys
        .iter()
        .map(|key| {
            format!(
                "<a href=\"#section-{}\">{}</a>",
                escape_html(key),
                escape_html(&section_title(key, collections))
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<section class=\"panel featured\"><div class=\"section-header\"><div><h2>Collections</h2><p>Jump between the major visualization areas for this project.</p></div></div><div class=\"nav-links\">{links}</div></section>"
    )
}

fn render_featured(diagrams: &[DiagramView]) -> String {
    let featured: Vec<_> = diagrams.iter().filter(|diagram| diagram.featured).collect();
    if featured.is_empty() {
        return String::new();
    }
    let cards = featured
        .into_iter()
        .map(render_card)
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<section class=\"panel featured\"><div class=\"section-header\"><div><h2>Featured pages</h2><p>Priority explainers and visual surfaces worth starting with.</p></div></div><div class=\"card-grid\">{cards}</div></section>"
    )
}

fn render_recent(diagrams: &[DiagramView]) -> String {
    if diagrams.is_empty() {
        return "<div class=\"list\"><div class=\"list-item\">No pages yet.</div></div>"
            .to_string();
    }
    let items = diagrams
        .iter()
        .map(|diagram| {
            format!(
                "<div class=\"list-item\"><strong><a href=\"{}\">{}</a></strong><div>{}</div><div class=\"badge-row\"><span class=\"badge\">{}</span><span class=\"badge\">{}</span></div></div>",
                escape_html(&diagram.page),
                escape_html(&diagram.title),
                escape_html(diagram.summary.as_deref().unwrap_or("No summary yet.")),
                escape_html(&diagram.section_title),
                escape_html(diagram.updated_at.as_deref().unwrap_or("untracked")),
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<div class=\"list\">{items}</div>")
}

fn render_sections(
    diagrams: &[DiagramView],
    section_keys: &[String],
    collections: &[CollectionRecord],
) -> String {
    let mut by_section: BTreeMap<String, Vec<&DiagramView>> = BTreeMap::new();
    for diagram in diagrams {
        by_section
            .entry(diagram.section.clone())
            .or_default()
            .push(diagram);
    }

    section_keys
        .iter()
        .map(|key| {
            let mut items = by_section.remove(key).unwrap_or_default();
            items.sort_by(|left, right| compare_diagrams(left, right));
            let count = items.len();
            let cards = if items.is_empty() {
                "<div class=\"empty-results\"><p>No pages in this collection yet.</p></div>".to_string()
            } else {
                items.into_iter().map(render_card).collect::<Vec<_>>().join("\n")
            };
            let title = section_title(key, collections);
            let description = section_description(key, collections)
                .unwrap_or_else(|| "AOC Map pages in this collection.".to_string());
            format!(
                "<section class=\"panel section-block\" id=\"section-{}\" data-section-block=\"{}\"><div class=\"section-header\"><div><h2>{}</h2><p>{}</p></div><div class=\"section-count\" data-section-count>{} pages</div></div><div class=\"card-grid\">{}</div></section>",
                escape_html(key),
                escape_html(key),
                escape_html(&title),
                escape_html(&description),
                count,
                cards,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_filter_buttons(group: &str, values: &[String]) -> String {
    values
        .iter()
        .map(|value| {
            format!(
                "<button class=\"chip\" type=\"button\" data-filter-group=\"{}\" data-filter-value=\"{}\">{}</button>",
                escape_html(group),
                escape_html(value),
                escape_html(&title_from_slug(value)),
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

fn render_card(diagram: &DiagramView) -> String {
    let tags = if diagram.tags.is_empty() {
        String::new()
    } else {
        format!(
            "<div class=\"tags\">{}</div>",
            diagram
                .tags
                .iter()
                .map(|tag| format!("<span class=\"tag\">{}</span>", escape_html(tag)))
                .collect::<Vec<_>>()
                .join("")
        )
    };
    let search = format!(
        "{} {} {} {} {} {}",
        diagram.title,
        diagram.summary.clone().unwrap_or_default(),
        diagram.section,
        diagram.kind,
        diagram.status,
        diagram.tags.join(" ")
    );
    let sources = if diagram.source_paths.is_empty() {
        "No sources listed".to_string()
    } else {
        format!("Sources: {}", diagram.source_paths.join(", "))
    };
    let generated_badge = if diagram.generated {
        "<span class=\"badge\">generated</span>"
    } else {
        ""
    };
    let inferred_badge = if diagram.inferred {
        "<span class=\"badge\">inferred</span>"
    } else {
        ""
    };

    format!(
        "<article class=\"card\" data-card=\"1\" data-section=\"{}\" data-kind=\"{}\" data-tags=\"{}\" data-search=\"{}\">\n  <div class=\"card-top\">\n    <div class=\"badge-row\">\n      <span class=\"badge\">{}</span>\n      <span class=\"badge kind\">{}</span>\n      <span class=\"badge status-{}\">{}</span>\n      {}{}\n    </div>\n  </div>\n  <div>\n    <h3><a href=\"{}\">{}</a></h3>\n    <p>{}</p>\n  </div>\n  {}\n  <div class=\"meta-list\">\n    <div><strong>Slug:</strong> {}</div>\n    <div><strong>Updated:</strong> {}</div>\n    <div><strong>{}</strong></div>\n  </div>\n  <div class=\"hero-actions\">\n    <a class=\"button\" href=\"{}\">Open page</a>\n  </div>\n</article>",
        escape_html(&diagram.section),
        escape_html(&diagram.kind),
        escape_html(&diagram.tags.join(" ")),
        escape_html(&search),
        escape_html(&diagram.section_title),
        escape_html(&title_from_slug(&diagram.kind)),
        escape_html(&diagram.status),
        escape_html(&title_from_slug(&diagram.status)),
        generated_badge,
        inferred_badge,
        escape_html(&diagram.page),
        escape_html(&diagram.title),
        escape_html(diagram.summary.as_deref().unwrap_or("No summary yet. Open the page for details.")),
        tags,
        escape_html(&diagram.slug),
        escape_html(diagram.updated_at.as_deref().unwrap_or("untracked")),
        escape_html(&sources),
        escape_html(&diagram.page),
    )
}

fn section_title(section: &str, collections: &[CollectionRecord]) -> String {
    collections
        .iter()
        .find(|collection| collection.key == section)
        .map(|collection| collection.title.clone())
        .unwrap_or_else(|| title_from_slug(section))
}

fn section_description(section: &str, collections: &[CollectionRecord]) -> Option<String> {
    collections
        .iter()
        .find(|collection| collection.key == section)
        .and_then(|collection| collection.description.clone())
}

fn starter_readme() -> String {
    "# AOC Map\n\nAOC Map is the project-local graph and visualization microsite for this repo. The main artifact is the graph, not the chrome around it.\n\n## Layout\n- `pages/*.html` — minimal graph-first presentation pages.\n- `diagrams/*.mmd` — Mermaid source files used as the canonical graph definitions.\n- `assets/mermaid.min.js` — vendored Mermaid runtime used locally/offline.\n- `assets/render-mermaid.js` — AOC Map helper that renders Mermaid blocks and Mermaid source files in the browser.\n- `manifest.json` — site metadata and page metadata used for the homepage shell.\n- `index.html` — generated homepage for the microsite.\n\n## Workflow\n1. `aoc-map init`\n2. `aoc-map new agent-topology --section agents --kind topology --summary \"How AOC agents route through this repo\"`\n3. Edit the generated Mermaid file under `diagrams/agent-topology.mmd`.\n4. Keep the page under `pages/agent-topology.html` minimal and graph-first.\n5. Rebuild with `aoc-map build`, then browse with `aoc-map serve --open`.\n\n## Metadata conventions\nPages can declare metadata directly in HTML via meta tags such as:\n- `<meta name=\"aoc-map:summary\" content=\"...\">`\n- `<meta name=\"aoc-map:section\" content=\"agents\">`\n- `<meta name=\"aoc-map:kind\" content=\"topology\">`\n- `<meta name=\"aoc-map:status\" content=\"active\">`\n- `<meta name=\"aoc-map:diagram\" content=\"diagrams/agent-topology.mmd\">`\n- `<meta name=\"aoc-map:tags\" content=\"agents,orchestration\">`\n\n## Graph authoring\nPrefer Mermaid files in `diagrams/*.mmd` and reference them from pages with:\n\n```html\n<script type=\"text/plain\" data-aoc-map-mermaid-src=\"../diagrams/agent-topology.mmd\"></script>\n```\n\nInline Mermaid blocks still work, but external graph files are the preferred project-context-friendly path.\n\nPrefer self-contained HTML/CSS/JS/SVG and avoid external network-loaded assets when possible.\n"
            .to_string()
}

fn starter_mermaid_source(title: &str) -> String {
    let label = title.replace('"', "'");
    format!(
        "flowchart LR\n    repo[Project context] --> graph[{label}]\n    graph --> page[Minimal AOC Map page]\n    graph --> refs[Code · tasks · docs]\n    refs --> next[Explore or explain]\n"
    )
}

fn starter_page_html(site: &SiteConfig, title: &str, record: &DiagramRecord, slug: &str) -> String {
    let site_title = site.title.clone().unwrap_or_else(|| "AOC Map".to_string());
    let summary = record.summary.clone().unwrap_or_else(|| {
        "Describe the graph, the flow it captures, and why it matters in the project.".to_string()
    });
    let section = record
        .section
        .clone()
        .unwrap_or_else(|| "explainers".to_string());
    let kind = record.kind.clone().unwrap_or_else(|| "explain".to_string());
    let status = record.status.clone().unwrap_or_else(|| "draft".to_string());
    let diagram_path = record
        .diagram_path
        .clone()
        .unwrap_or_else(|| format!("diagrams/{slug}.mmd"));
    let diagram_src = format!("../{diagram_path}");
    let tags = if record.tags.is_empty() {
        "".to_string()
    } else {
        record.tags.join(",")
    };
    let source_list = if record.source_paths.is_empty() {
        "<li><code>Add file paths, task IDs, or commands here.</code></li>".to_string()
    } else {
        record
            .source_paths
            .iter()
            .map(|source| format!("<li><code>{}</code></li>", escape_html(source)))
            .collect::<Vec<_>>()
            .join("")
    };

    format!(
        r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <meta name="description" content="{summary}">
  <meta name="aoc-map:summary" content="{summary}">
  <meta name="aoc-map:section" content="{section}">
  <meta name="aoc-map:kind" content="{kind}">
  <meta name="aoc-map:status" content="{status}">
  <meta name="aoc-map:diagram" content="{diagram_path}">
  <meta name="aoc-map:tags" content="{tags}">
  <script defer src="../assets/mermaid.min.js"></script>
  <script defer src="../assets/render-mermaid.js"></script>
  <style>
    :root {{
      color-scheme: dark;
      --bg: #09111b;
      --panel: #0f1724;
      --line: #223352;
      --text: #eef4ff;
      --muted: #9cb0d0;
      --accent: #79c0ff;
    }}
    * {{ box-sizing: border-box; }}
    body {{ margin: 0; font-family: Inter, ui-sans-serif, system-ui, sans-serif; background: var(--bg); color: var(--text); }}
    a {{ color: var(--accent); }}
    code {{ background: #09111b; border: 1px solid var(--line); border-radius: 8px; padding: 0.12rem 0.4rem; color: var(--accent); }}
    main {{ max-width: 1240px; margin: 0 auto; padding: 22px 18px 40px; }}
    .top {{ display: flex; justify-content: space-between; gap: 12px; flex-wrap: wrap; align-items: center; margin-bottom: 18px; }}
    .meta {{ display: flex; flex-wrap: wrap; gap: 8px; }}
    .pill {{ padding: 6px 10px; border-radius: 999px; border: 1px solid var(--line); color: var(--muted); background: #0b1320; }}
    .layout {{ display: grid; gap: 18px; grid-template-columns: minmax(0, 1.9fr) minmax(280px, 0.9fr); }}
    .panel {{ background: var(--panel); border: 1px solid var(--line); border-radius: 18px; padding: 18px; }}
    h1 {{ margin: 0 0 8px; font-size: clamp(2rem, 4vw, 3rem); line-height: 1.05; }}
    h2, h3 {{ margin: 0 0 10px; }}
    p, li {{ color: var(--muted); line-height: 1.6; }}
    .graph-shell {{ min-height: 72vh; display: flex; flex-direction: column; gap: 14px; }}
    .graph-frame {{ flex: 1; min-height: 520px; border-radius: 14px; border: 1px solid var(--line); background: #0a1220; padding: 12px; overflow: hidden; }}
    .mermaid-rendered {{ width: 100%; height: 100%; overflow: auto; }}
    .mermaid-rendered svg {{ width: 100%; height: auto; display: block; min-width: 720px; }}
    .stack {{ display: grid; gap: 16px; align-content: start; }}
    .stack ul {{ margin: 0; padding-left: 18px; }}
    .footer {{ margin-top: 16px; color: var(--muted); }}
    @media (max-width: 940px) {{ .layout {{ grid-template-columns: 1fr; }} .graph-frame {{ min-height: 420px; }} }}
  </style>
</head>
<body>
  <main>
    <div class="top">
      <a href="../index.html">← Back to {site_title}</a>
      <div class="meta">
        <span class="pill">section: {section}</span>
        <span class="pill">kind: {kind}</span>
        <span class="pill">status: {status}</span>
        <span class="pill">slug: {slug}</span>
      </div>
    </div>

    <div class="layout">
      <section class="panel graph-shell">
        <div>
          <h1>{title}</h1>
          <p>{summary}</p>
        </div>
        <div class="graph-frame">
          <script type="text/plain" data-aoc-map-mermaid-src="{diagram_src}"></script>
        </div>
        <div class="footer">Primary graph source: <code>{diagram_path}</code>. Edit that Mermaid file, then run <code>aoc-map build</code> and preview with <code>aoc-map serve</code>.</div>
      </section>

      <aside class="stack">
        <article class="panel">
          <h3>Focus</h3>
          <ul>
            <li>Keep the graph as the main artifact.</li>
            <li>Use minimal prose around the flow.</li>
            <li>Prefer one graph question per page.</li>
          </ul>
        </article>
        <article class="panel">
          <h3>Source references</h3>
          <ul>{source_list}</ul>
        </article>
        <article class="panel">
          <h3>Editing notes</h3>
          <ul>
            <li>Store Mermaid source in <code>{diagram_path}</code>.</li>
            <li>Keep metadata in the page head with <code>aoc-map:*</code> tags.</li>
            <li>Inline Mermaid blocks still work when needed.</li>
          </ul>
        </article>
      </aside>
    </div>
  </main>
</body>
</html>
"##,
        site_title = escape_html(&site_title),
        title = escape_html(title),
        summary = escape_html(&summary),
        section = escape_html(&section),
        kind = escape_html(&kind),
        status = escape_html(&status),
        slug = escape_html(slug),
        tags = escape_html(&tags),
        diagram_path = escape_html(&diagram_path),
        diagram_src = escape_html(&diagram_src),
        source_list = source_list,
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn find_python() -> Option<&'static str> {
    for candidate in ["python3", "python"] {
        let Ok(status) = Command::new(candidate).arg("--version").status() else {
            continue;
        };
        if status.success() {
            return Some(candidate);
        }
    }
    None
}

fn try_open_browser(url: &str) -> Result<()> {
    let commands: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("open", &[url])]
    } else {
        &[("xdg-open", &[url])]
    };
    for (command, args) in commands {
        if let Ok(status) = Command::new(command).args(*args).status() {
            if status.success() {
                return Ok(());
            }
        }
    }
    bail!("failed to open browser automatically")
}

#[cfg(test)]
mod tests {
    use super::{
        infer_section, normalize_key, normalize_mermaid_page_html, normalize_tokens, validate_slug,
        MERMAID_OUTPUT_END, MERMAID_OUTPUT_START,
    };

    #[test]
    fn slug_validation_accepts_expected_format() {
        assert_eq!(validate_slug("agent-topology").unwrap(), "agent-topology");
        assert!(validate_slug("AgentTopology").is_err());
        assert!(validate_slug("bad_slug").is_err());
    }

    #[test]
    fn normalize_key_collapses_spacing_and_case() {
        assert_eq!(normalize_key("Agent Topology"), "agent-topology");
        assert_eq!(normalize_key("mind__ops"), "mind-ops");
    }

    #[test]
    fn normalize_tokens_dedupes_and_sorts() {
        assert_eq!(
            normalize_tokens(vec!["Ops".into(), "ops".into(), "Mind Flow".into()]),
            vec!["mind-flow".to_string(), "ops".to_string()]
        );
    }

    #[test]
    fn section_inference_prefers_dashboard_and_topology_defaults() {
        assert_eq!(infer_section(Some("dashboard")), "dashboards");
        assert_eq!(infer_section(Some("topology")), "agents");
        assert_eq!(infer_section(Some("flow")), "explainers");
    }

    #[test]
    fn mermaid_pages_get_local_script_tags_and_strip_legacy_output() {
        let input = format!(
            "<html><head><title>x</title></head><body><section><script type=\"text/plain\" data-aoc-map-mermaid>flowchart LR; A-->B</script>\n{MERMAID_OUTPUT_START}<div>old</div>{MERMAID_OUTPUT_END}</section></body></html>"
        );
        let output = normalize_mermaid_page_html(&input);
        assert!(output.contains("data-aoc-map-mermaid"));
        assert!(output.contains("../assets/mermaid.min.js"));
        assert!(output.contains("../assets/render-mermaid.js"));
        assert!(!output.contains(MERMAID_OUTPUT_START));
        assert!(!output.contains(MERMAID_OUTPUT_END));
        assert!(!output.contains("<div>old</div>"));
    }

    #[test]
    fn mermaid_script_injection_is_idempotent() {
        let input = "<html><head><title>x</title></head><body><script type=\"text/plain\" data-aoc-map-mermaid>flowchart LR; A-->B</script></body></html>";
        let once = normalize_mermaid_page_html(input);
        let twice = normalize_mermaid_page_html(&once);
        assert_eq!(once.matches("mermaid.min.js").count(), 1);
        assert_eq!(once.matches("render-mermaid.js").count(), 1);
        assert_eq!(once, twice);
    }

    #[test]
    fn mermaid_src_blocks_also_get_local_script_tags() {
        let input = "<html><head><title>x</title></head><body><script type=\"text/plain\" data-aoc-map-mermaid-src=\"../diagrams/agent-topology.mmd\"></script></body></html>";
        let output = normalize_mermaid_page_html(input);
        assert!(output.contains("data-aoc-map-mermaid-src"));
        assert!(output.contains("../assets/mermaid.min.js"));
        assert!(output.contains("../assets/render-mermaid.js"));
    }
}

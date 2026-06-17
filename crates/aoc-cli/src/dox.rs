use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

const SCHEMA_VERSION: &str = "aoc.dox.v1";
const DOX_DIR: &str = ".aoc/dox";
const MAP_PATH: &str = ".aoc/dox/map.json";
const CANDIDATES_PATH: &str = ".aoc/dox/candidates.json";
const ROUTES_PATH: &str = ".aoc/dox/routes.json";
const BUDGETS_PATH: &str = ".aoc/dox/budgets.json";
const REPORT_PATH: &str = ".aoc/dox/report.md";
const REVIEW_PACKET_PATH: &str = ".aoc/dox/review.md";
const EVAL_MATRIX_PATH: &str = ".aoc/dox/eval-matrix.json";
const ROOT_AGENTS: &str = "AGENTS.md";
const CODEGRAPH_ERRORS: &str = ".codegraph/errors.log";
const ROOT_AGENTS_TARGET_BYTES: u32 = 8192;
const ROOT_AGENTS_HARD_BYTES: u32 = 12288;
const CHILD_AGENTS_TARGET_BYTES: u32 = 2048;
const CHILD_AGENTS_HARD_BYTES: u32 = 4096;
const GENERATED_SCAN_LIMIT: u64 = 32 * 1024;

#[derive(Subcommand, Debug)]
#[command(rename_all = "kebab-case")]
pub enum DoxCommand {
    /// Scan the repo and write/update .aoc/dox metadata without writing AGENTS.md files
    Map(MapArgs),
    /// Print candidate decisions and budget status from .aoc/dox/candidates.json
    Review(ReviewArgs),
    /// Apply approved AGENTS.md writes from .aoc/dox/candidates.json
    Apply(ApplyArgs),
    /// Validate existing .aoc/dox metadata and AGENTS.md chain health
    Doctor(DoctorArgs),
    /// Emit the eval matrix scaffold for comparing context strategies
    Eval(EvalArgs),
}

#[derive(Args, Debug, Clone)]
pub struct MapArgs {
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub no_codegraph: bool,
    #[arg(long, default_value_t = 7)]
    pub min_score: i32,
    #[arg(long, default_value_t = 12_000)]
    pub max_codegraph_chars: u32,
    #[arg(long, default_value_t = 16_384)]
    pub active_chain_target_bytes: u32,
    #[arg(long, default_value_t = 24_576)]
    pub active_chain_hard_bytes: u32,
}

#[derive(Args, Debug, Clone)]
pub struct ReviewArgs {
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub packet: bool,
    #[arg(long)]
    pub write_packet: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ApplyArgs {
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub yes: bool,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub include_content: bool,
}

#[derive(Args, Debug, Clone)]
pub struct DoctorArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct EvalArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DoxEnvelope<T> {
    schema: String,
    generated_at: String,
    project_root: String,
    data: T,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DoxCandidate {
    path: String,
    decision: CandidateDecision,
    score: i32,
    confidence: f32,
    reason: String,
    contracts: Vec<LocalContract>,
    risks: Vec<String>,
    verification: Vec<String>,
    evidence: Vec<EvidenceRef>,
    target_agents_path: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CandidateDecision {
    Create,
    Update,
    Reject,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct LocalContract {
    rule: String,
    do_not: Vec<String>,
    update_when: Vec<String>,
    verification: Vec<String>,
    evidence: Vec<EvidenceRef>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct EvidenceRef {
    path: String,
    symbol: Option<String>,
    command: Option<String>,
    note: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DoxDirectoryCoverage {
    path: String,
    kind: DirectoryKind,
    resolved_agents_chain: Vec<String>,
    effective_agents_bytes: u64,
    coverage: CoverageLevel,
    status: CoverageStatus,
    candidate_path: Option<String>,
    missing_contracts: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum DirectoryKind {
    Package,
    App,
    Source,
    Tests,
    Config,
    Scripts,
    Docs,
    Generated,
    Cache,
    Other,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CoverageLevel {
    RootOnly,
    Inherited,
    Specific,
    Insufficient,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CoverageStatus {
    Ok,
    CandidateLocalAgents,
    OverBudget,
    StaleEvidence,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DoxRoute {
    path_glob: String,
    agent_profile: String,
    required_context: Vec<String>,
    verification: Vec<String>,
    source_candidate_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DoxBudgets {
    root_agents_target_bytes: u32,
    root_agents_hard_bytes: u32,
    child_agents_target_bytes: u32,
    child_agents_hard_bytes: u32,
    active_chain_target_bytes: u32,
    active_chain_hard_bytes: u32,
    measured_root_agents_bytes: u64,
    measured_project_chain_bytes: u64,
    fallback_filenames_supported: bool,
    status: BudgetStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum BudgetStatus {
    Ok,
    OverTarget,
    OverHard,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DoxMapData {
    codegraph: CodeGraphSummary,
    directories: Vec<String>,
    package_manifests: Vec<String>,
    instruction_files: Vec<String>,
    test_configs: Vec<String>,
    generated_markers: Vec<GeneratedMarker>,
    coverage: Vec<DoxDirectoryCoverage>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CodeGraphSummary {
    available: bool,
    disabled: bool,
    commands: Vec<CommandSummary>,
    errors_log: Option<CodeGraphErrorsSummary>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CommandSummary {
    command: String,
    status: Option<i32>,
    success: bool,
    truncated: bool,
    stdout_chars: usize,
    stderr_chars: usize,
    summary: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CodeGraphErrorsSummary {
    non_empty_lines: usize,
    first_paths: Vec<String>,
    any_path_missing: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GeneratedMarker {
    path: String,
    marker: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DoxCandidatesData {
    candidates: Vec<DoxCandidate>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DoxRoutesData {
    routes: Vec<DoxRoute>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct EvalMatrixData {
    strategies: Vec<String>,
    metrics: Vec<String>,
}

pub fn handle_dox_command(action: DoxCommand) -> Result<()> {
    match action {
        DoxCommand::Map(args) => handle_map(args),
        DoxCommand::Review(args) => handle_review(args),
        DoxCommand::Apply(args) => handle_apply(args),
        DoxCommand::Doctor(args) => handle_doctor(args),
        DoxCommand::Eval(args) => handle_eval(args),
    }
}

fn handle_map(args: MapArgs) -> Result<()> {
    let project_root = std::env::current_dir().context("resolve project root")?;
    fs::create_dir_all(project_root.join(DOX_DIR)).context("create .aoc/dox")?;

    let scan = deterministic_scan(&project_root)?;
    let codegraph = collect_codegraph_summary(&project_root, args.no_codegraph, args.max_codegraph_chars);
    let mut candidates = build_candidates(&project_root, &scan, args.min_score)?;
    let coverage = build_coverage(&project_root, &scan.directories, &candidates, &args)?;
    mark_coverage_candidates(&coverage, &mut candidates);
    let budgets = build_budgets(&project_root, args.active_chain_target_bytes, args.active_chain_hard_bytes)?;
    let routes = build_routes(&candidates);

    let map = DoxMapData {
        codegraph,
        directories: scan.directories.iter().map(|path| rel_path(&project_root, path)).collect(),
        package_manifests: scan.package_manifests,
        instruction_files: scan.instruction_files,
        test_configs: scan.test_configs,
        generated_markers: scan.generated_markers,
        coverage,
    };
    let map_env = envelope(&project_root, map);
    let candidates_env = envelope(&project_root, DoxCandidatesData { candidates });
    let routes_env = envelope(&project_root, DoxRoutesData { routes });
    let budgets_env = envelope(&project_root, budgets);

    write_json(project_root.join(MAP_PATH), &map_env)?;
    write_json(project_root.join(CANDIDATES_PATH), &candidates_env)?;
    write_json(project_root.join(ROUTES_PATH), &routes_env)?;
    write_json(project_root.join(BUDGETS_PATH), &budgets_env)?;
    fs::write(project_root.join(REPORT_PATH), render_report(&map_env, &candidates_env, &budgets_env)?)
        .context("write dox report")?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&map_env)?);
    } else {
        println!(
            "AOC DOX map written: {}, {}, {}, {}, {}",
            MAP_PATH, CANDIDATES_PATH, ROUTES_PATH, BUDGETS_PATH, REPORT_PATH
        );
    }
    Ok(())
}

fn handle_review(args: ReviewArgs) -> Result<()> {
    let project_root = std::env::current_dir().context("resolve project root")?;
    let map: DoxEnvelope<DoxMapData> = read_json(project_root.join(MAP_PATH))?;
    let candidates: DoxEnvelope<DoxCandidatesData> = read_json(project_root.join(CANDIDATES_PATH))?;
    let budgets: DoxEnvelope<DoxBudgets> = read_json(project_root.join(BUDGETS_PATH))?;
    let routes: DoxEnvelope<DoxRoutesData> = read_json(project_root.join(ROUTES_PATH))?;
    if args.write_packet && !args.packet {
        bail!("--write-packet requires --packet");
    }

    if args.packet {
        let packet = render_review_packet(&map, &candidates, &budgets, &routes)?;
        let written = args.write_packet;
        if written {
            let packet_path = project_root.join(REVIEW_PACKET_PATH);
            if let Some(parent) = packet_path.parent() {
                fs::create_dir_all(parent).with_context(|| format!("create parent for {}", REVIEW_PACKET_PATH))?;
            }
            fs::write(&packet_path, &packet).with_context(|| format!("write {}", REVIEW_PACKET_PATH))?;
        }
        if args.json {
            let value = serde_json::json!({
                "schema": SCHEMA_VERSION,
                "packet_path": REVIEW_PACKET_PATH,
                "packet": packet,
                "written": written,
            });
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!("{}", packet);
        }
        return Ok(());
    }

    if args.json {
        let value = serde_json::json!({
            "schema": SCHEMA_VERSION,
            "map": map,
            "candidates": candidates,
            "budgets": budgets,
            "routes": routes,
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }

    println!("AOC DOX review");
    println!("Budget status: {:?}", budgets.data.status);
    for decision in [CandidateDecision::Create, CandidateDecision::Update, CandidateDecision::Reject] {
        println!("\n{:?}", decision);
        let mut group: Vec<&DoxCandidate> = candidates
            .data
            .candidates
            .iter()
            .filter(|candidate| candidate.decision == decision)
            .collect();
        group.sort_by_key(|candidate| (Reverse(candidate.score), candidate.path.clone()));
        for candidate in group {
            println!("- {} score={} reason={}", candidate.path, candidate.score, candidate.reason);
        }
    }
    Ok(())
}

fn handle_apply(args: ApplyArgs) -> Result<()> {
    if args.include_content && (!args.dry_run || !args.json) {
        bail!("--include-content is only valid with --dry-run --json");
    }

    let project_root = std::env::current_dir().context("resolve project root")?;
    let candidates: DoxEnvelope<DoxCandidatesData> = read_json(project_root.join(CANDIDATES_PATH))?;
    let selected: Vec<&DoxCandidate> = candidates
        .data
        .candidates
        .iter()
        .filter(|candidate| matches!(candidate.decision, CandidateDecision::Create | CandidateDecision::Update))
        .collect();

    let mut rendered = Vec::new();
    for candidate in selected {
        let target = candidate
            .target_agents_path
            .as_ref()
            .ok_or_else(|| anyhow!("approved candidate missing target_agents_path: {}", candidate.path))?;
        let content = render_agents_file(candidate)?;
        rendered.push((target.clone(), content));
    }

    if args.dry_run {
        if args.json {
            let items: Vec<_> = rendered
                .iter()
                .map(|(path, content)| {
                    let mut item = serde_json::Map::new();
                    item.insert("path".to_string(), serde_json::json!(path));
                    item.insert("bytes".to_string(), serde_json::json!(content.len()));
                    if args.include_content {
                        item.insert("content".to_string(), serde_json::json!(content));
                    }
                    serde_json::Value::Object(item)
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "schema": SCHEMA_VERSION, "dry_run": true, "targets": items }))?);
        } else {
            println!("AOC DOX apply dry-run");
            for (path, content) in &rendered {
                println!("- {} ({} bytes)", path, content.len());
            }
        }
        return Ok(());
    }

    if !args.yes {
        for (path, content) in &rendered {
            println!("- {} ({} bytes)", path, content.len());
        }
        bail!("Refusing to write AGENTS.md files without --yes; rerun with --dry-run to inspect or --yes to apply.");
    }

    let mut written = Vec::new();
    for (target, content) in rendered {
        let target_path = project_root.join(&target);
        if let Ok(existing) = fs::read_to_string(&target_path) {
            if existing != content {
                bail!("existing AGENTS.md has unmanaged content; rerun mapping with this file as evidence");
            }
        }
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create parent for {}", target))?;
        }
        fs::write(&target_path, content).with_context(|| format!("write {}", target))?;
        written.push(target);
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "schema": SCHEMA_VERSION, "written": written }))?);
    } else {
        for path in written {
            println!("wrote {}", path);
        }
    }
    Ok(())
}


fn handle_doctor(args: DoctorArgs) -> Result<()> {
    let project_root = std::env::current_dir().context("resolve project root")?;
    let map: DoxEnvelope<DoxMapData> = read_json(project_root.join(MAP_PATH))?;
    let candidates: DoxEnvelope<DoxCandidatesData> = read_json(project_root.join(CANDIDATES_PATH))?;
    let budgets: DoxEnvelope<DoxBudgets> = read_json(project_root.join(BUDGETS_PATH))?;
    let routes: DoxEnvelope<DoxRoutesData> = read_json(project_root.join(ROUTES_PATH))?;

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    validate_schema(&map.schema, "map", &mut errors);
    validate_schema(&candidates.schema, "candidates", &mut errors);
    validate_schema(&budgets.schema, "budgets", &mut errors);
    validate_schema(&routes.schema, "routes", &mut errors);
    validate_candidates(&project_root, &candidates.data.candidates, &mut errors);
    for command in all_metadata_commands(&map, &candidates, &routes) {
        if let Err(error) = validate_verification_command(&command) {
            errors.push(error.to_string());
        }
    }
    for candidate in &candidates.data.candidates {
        if matches!(candidate.decision, CandidateDecision::Create | CandidateDecision::Update) {
            match render_agents_file(candidate) {
                Ok(content) if content.len() as u32 > CHILD_AGENTS_HARD_BYTES => errors.push(format!(
                    "generated child AGENTS.md exceeds hard budget: {}",
                    candidate.path
                )),
                Err(error) => errors.push(error.to_string()),
                _ => {}
            }
        }
    }
    if budgets.data.status == BudgetStatus::OverHard {
        errors.push("active AGENTS chain exceeds hard budget".to_string());
    } else if budgets.data.status == BudgetStatus::OverTarget {
        warnings.push("active AGENTS chain is over target budget".to_string());
    }

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "schema": SCHEMA_VERSION,
                "ok": errors.is_empty(),
                "warnings": warnings,
                "errors": errors,
            }))?
        );
    } else {
        for warning in &warnings {
            println!("warning: {}", warning);
        }
        for error in &errors {
            println!("error: {}", error);
        }
        if errors.is_empty() {
            println!("AOC DOX doctor ok");
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        bail!("AOC DOX doctor found {} error(s)", errors.len())
    }
}

fn handle_eval(args: EvalArgs) -> Result<()> {
    let project_root = std::env::current_dir().context("resolve project root")?;
    fs::create_dir_all(project_root.join(DOX_DIR)).context("create .aoc/dox")?;
    let data = EvalMatrixData {
        strategies: ["no_agents", "root_agents_only", "sparse_dox", "bloated_dox"]
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        metrics: [
            "task_success",
            "tests_passing",
            "tool_calls",
            "input_tokens",
            "output_tokens",
            "wall_time_ms",
            "irrelevant_files_touched",
            "docs_changed",
            "stale_or_false_rules",
            "human_review_severity",
        ]
        .iter()
        .map(|value| (*value).to_string())
        .collect(),
    };
    let env = envelope(&project_root, data);
    write_json(project_root.join(EVAL_MATRIX_PATH), &env)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&env)?);
    } else {
        println!("Eval matrix scaffold written; task runner integration is not implemented in v1.");
    }
    Ok(())
}

fn envelope<T>(project_root: &Path, data: T) -> DoxEnvelope<T> {
    DoxEnvelope {
        schema: SCHEMA_VERSION.to_string(),
        generated_at: Utc::now().to_rfc3339(),
        project_root: project_root.display().to_string(),
        data,
    }
}

fn write_json<T: Serialize>(path: PathBuf, value: &T) -> Result<()> {
    let content = format!("{}\n", serde_json::to_string_pretty(value)?);
    fs::write(&path, content).with_context(|| format!("write {}", path.display()))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<T> {
    let content = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("parse {}", path.display()))
}

struct ScanFacts {
    directories: Vec<PathBuf>,
    package_manifests: Vec<String>,
    instruction_files: Vec<String>,
    test_configs: Vec<String>,
    generated_markers: Vec<GeneratedMarker>,
}

fn deterministic_scan(project_root: &Path) -> Result<ScanFacts> {
    let mut directories = BTreeSet::new();
    directories.insert(project_root.to_path_buf());
    let mut package_manifests = BTreeSet::new();
    let mut instruction_files = BTreeSet::new();
    let mut test_configs = BTreeSet::new();
    let mut generated_markers = Vec::new();
    scan_dir(
        project_root,
        project_root,
        0,
        &mut directories,
        &mut package_manifests,
        &mut instruction_files,
        &mut test_configs,
        &mut generated_markers,
    )?;
    Ok(ScanFacts {
        directories: directories.into_iter().collect(),
        package_manifests: package_manifests.into_iter().collect(),
        instruction_files: instruction_files.into_iter().collect(),
        test_configs: test_configs.into_iter().collect(),
        generated_markers,
    })
}

fn scan_dir(
    project_root: &Path,
    dir: &Path,
    depth: usize,
    directories: &mut BTreeSet<PathBuf>,
    package_manifests: &mut BTreeSet<String>,
    instruction_files: &mut BTreeSet<String>,
    test_configs: &mut BTreeSet<String>,
    generated_markers: &mut Vec<GeneratedMarker>,
) -> Result<()> {
    if depth > 6 {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("read directory {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        if should_exclude(&path, &file_name) {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            directories.insert(path.clone());
            scan_dir(
                project_root,
                &path,
                depth + 1,
                directories,
                package_manifests,
                instruction_files,
                test_configs,
                generated_markers,
            )?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let rel = rel_path(project_root, &path);
        if is_package_manifest(&file_name) {
            package_manifests.insert(rel.clone());
        }
        if is_instruction_file(project_root, &path, &file_name) {
            instruction_files.insert(rel.clone());
        }
        if is_test_config(&file_name, &rel) {
            test_configs.insert(rel.clone());
        }
        if let Some(marker) = generated_marker(&path)? {
            generated_markers.push(GeneratedMarker { path: rel, marker });
        }
    }
    Ok(())
}

fn should_exclude(path: &Path, file_name: &str) -> bool {
    matches!(
        file_name,
        ".git"
            | ".jj"
            | "node_modules"
            | ".next"
            | ".codegraph"
            | "target"
            | "dist"
            | "build"
            | "coverage"
            | ".turbo"
            | ".cache"
    ) || path.ends_with(DOX_DIR)
}

fn is_package_manifest(file_name: &str) -> bool {
    matches!(
        file_name,
        "package.json" | "Cargo.toml" | "pyproject.toml" | "go.mod" | "deno.json" | "bun.lock" | "pnpm-lock.yaml"
    )
}

fn is_instruction_file(project_root: &Path, path: &Path, file_name: &str) -> bool {
    matches!(file_name, "AGENTS.md" | "AGENTS.override.md" | ".cursorrules" | ".clinerules")
        || rel_path(project_root, path) == ".github/copilot-instructions.md"
        || rel_path(project_root, path).starts_with(".cursor/rules/")
        || rel_path(project_root, path).starts_with(".codex/agents/")
        || rel_path(project_root, path).starts_with(".omp/agents/")
}

fn is_test_config(file_name: &str, rel: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.contains("vitest")
        || lower.contains("jest")
        || lower.contains("pytest")
        || lower.contains("playwright")
        || lower.contains("cypress")
        || rel == "package.json"
        || rel.ends_with("/package.json")
}

fn generated_marker(path: &Path) -> Result<Option<String>> {
    let Ok(mut file) = fs::File::open(path) else {
        return Ok(None);
    };
    let mut limited = (&mut file).take(GENERATED_SCAN_LIMIT);
    let mut bytes = Vec::new();
    limited.read_to_end(&mut bytes)?;
    if bytes.contains(&0) {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&bytes);
    for marker in ["@generated", "Code generated", "DO NOT EDIT", "Generated from", "aoc-managed"] {
        if text.contains(marker) {
            return Ok(Some(marker.to_string()));
        }
    }
    Ok(None)
}

fn collect_codegraph_summary(project_root: &Path, disabled: bool, max_chars: u32) -> CodeGraphSummary {
    let db_exists = project_root.join(".codegraph/codegraph.db").exists();
    let mut commands = Vec::new();
    if db_exists && !disabled {
        commands.push(run_codegraph(project_root, &["status", ".", "--json"], max_chars));
        commands.push(run_codegraph(
            project_root,
            &["files", "--path", ".", "--max-depth", "3", "--json"],
            max_chars,
        ));
    }
    CodeGraphSummary {
        available: db_exists && !disabled && commands.iter().any(|cmd| cmd.success),
        disabled,
        commands,
        errors_log: summarize_codegraph_errors(project_root).ok().flatten(),
    }
}

fn run_codegraph(project_root: &Path, args: &[&str], max_chars: u32) -> CommandSummary {
    let command = format!("codegraph {}", args.join(" "));
    match Command::new("codegraph").args(args).current_dir(project_root).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let total = stdout.len() + stderr.len();
            CommandSummary {
                command,
                status: output.status.code(),
                success: output.status.success(),
                truncated: total > max_chars as usize,
                stdout_chars: stdout.len(),
                stderr_chars: stderr.len(),
                summary: Some(truncate_summary(&stdout, max_chars as usize)),
            }
        }
        Err(error) => CommandSummary {
            command,
            status: None,
            success: false,
            truncated: false,
            stdout_chars: 0,
            stderr_chars: error.to_string().len(),
            summary: Some(error.to_string()),
        },
    }
}

fn truncate_summary(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    let char_count = trimmed.chars().count();
    if char_count <= max_chars {
        trimmed.to_string()
    } else {
        format!("{}\n[truncated]", trimmed.chars().take(max_chars).collect::<String>())
    }
}

fn summarize_codegraph_errors(project_root: &Path) -> Result<Option<CodeGraphErrorsSummary>> {
    let path = project_root.join(CODEGRAPH_ERRORS);
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    let mut first_paths = Vec::new();
    let mut any_path_missing = false;
    let mut non_empty_lines = 0;
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        non_empty_lines += 1;
        if first_paths.len() >= 5 {
            continue;
        }
        if let Some(path) = first_path_like_token(line) {
            if !project_root.join(&path).exists() {
                any_path_missing = true;
            }
            first_paths.push(path);
        }
    }
    Ok(Some(CodeGraphErrorsSummary {
        non_empty_lines,
        first_paths,
        any_path_missing,
    }))
}

fn first_path_like_token(line: &str) -> Option<String> {
    line.split(|ch: char| ch.is_whitespace() || ch == ':' || ch == ',')
        .find(|token| token.contains('/') && !token.starts_with("http"))
        .map(|token| token.trim_matches(|ch| ch == '"' || ch == '\'').to_string())
}

fn build_candidates(project_root: &Path, scan: &ScanFacts, min_score: i32) -> Result<Vec<DoxCandidate>> {
    let mut candidates = Vec::new();
    for dir in &scan.directories {
        let rel = rel_path(project_root, dir);
        let exact_agents = local_agents_file(dir);
        let existing_rule = exact_agents.as_ref().and_then(|path| fs::read_to_string(path).ok());
        let mut evidence = Vec::new();
        let mut contracts = Vec::new();
        if rel != "." {
            if let (Some(path), Some(content)) = (&exact_agents, existing_rule.as_deref()) {
            let evidence_ref = EvidenceRef {
                path: rel_path(project_root, path),
                symbol: None,
                command: None,
                note: Some("existing local instruction file".to_string()),
            };
            evidence.push(evidence_ref.clone());
            for rule in durable_rule_lines(content) {
                contracts.push(LocalContract {
                    rule,
                    do_not: Vec::new(),
                    update_when: vec!["local contract changes".to_string()],
                    verification: verification_for_path(&rel),
                    evidence: vec![evidence_ref.clone()],
                });
            }
        }
        }
        let risks = risks_for_path(&rel, scan);
        let verification = verification_for_path(&rel);
        let decision = score_candidate(
            &rel,
            rel != "." && existing_rule.is_some(),
            !contracts.is_empty(),
            &risks,
            &verification,
            &evidence,
            min_score,
        );
        let target_agents_path = if matches!(decision.decision, CandidateDecision::Create | CandidateDecision::Update) {
            Some(if rel == "." { ROOT_AGENTS.to_string() } else { format!("{}/{}", rel, ROOT_AGENTS) })
        } else {
            None
        };
        candidates.push(DoxCandidate {
            path: rel,
            decision: decision.decision,
            score: decision.score,
            confidence: decision.confidence,
            reason: decision.reason,
            contracts,
            risks,
            verification,
            evidence,
            target_agents_path,
        });
    }
    candidates.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(candidates)
}

struct ScoreDecision {
    decision: CandidateDecision,
    score: i32,
    confidence: f32,
    reason: String,
}

fn score_candidate(
    path: &str,
    existing_local_rule_differs: bool,
    has_contracts: bool,
    risks: &[String],
    verification: &[String],
    evidence: &[EvidenceRef],
    min_score: i32,
) -> ScoreDecision {
    let mut score = 0;
    if existing_local_rule_differs {
        score += 3;
    }
    if risks.iter().any(|risk| risk == "high-risk invariant") {
        score += 3;
    }
    if verification.iter().any(|cmd| cmd.contains("test") || cmd.contains("check")) {
        score += 2;
    }
    if is_public_surface(path) {
        score += 2;
    }
    if is_dynamic_surface(path) {
        score += 2;
    }
    if is_obvious_layout_rule(path) {
        score -= 3;
    }
    if verification.is_empty() {
        score -= 2;
    }

    if score < min_score {
        return ScoreDecision {
            decision: CandidateDecision::Reject,
            score,
            confidence: 0.45,
            reason: "below min_score".to_string(),
        };
    }
    if evidence.is_empty() || verification.is_empty() {
        return ScoreDecision {
            decision: CandidateDecision::Reject,
            score,
            confidence: 0.5,
            reason: "missing evidence or verification".to_string(),
        };
    }
    if !has_contracts {
        return ScoreDecision {
            decision: CandidateDecision::Reject,
            score,
            confidence: 0.55,
            reason: "requires OMP mapper contract extraction".to_string(),
        };
    }
    ScoreDecision {
        decision: if existing_local_rule_differs { CandidateDecision::Update } else { CandidateDecision::Create },
        score,
        confidence: 0.8,
        reason: "evidence-backed local contract meets score threshold".to_string(),
    }
}

fn durable_rule_lines(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.trim_start_matches('-').trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

fn local_agents_file(dir: &Path) -> Option<PathBuf> {
    let override_path = dir.join("AGENTS.override.md");
    if non_empty_file(&override_path) {
        return Some(override_path);
    }
    let agents_path = dir.join(ROOT_AGENTS);
    if non_empty_file(&agents_path) {
        return Some(agents_path);
    }
    None
}

fn risks_for_path(rel: &str, scan: &ScanFacts) -> Vec<String> {
    let mut risks = Vec::new();
    if is_high_risk_path(rel) || scan.generated_markers.iter().any(|marker| marker.path.starts_with(rel.trim_start_matches("./"))) {
        risks.push("high-risk invariant".to_string());
    }
    risks
}

fn is_high_risk_path(rel: &str) -> bool {
    let rel = rel.trim_start_matches("./");
    rel.starts_with(".aoc/")
        || rel.starts_with(".taskmaster/")
        || rel.starts_with(".omp/")
        || rel.starts_with("scripts/")
        || rel.starts_with("bin/")
        || rel.contains("/auth")
        || rel.contains("auth/")
        || rel.contains("/migrations/")
        || rel.contains("/schema")
        || rel.contains("schema/")
        || rel.contains("/generated/")
        || rel.contains("/crons/")
        || rel.contains("/tools/")
        || rel.contains("/extensions/")
        || rel.contains("/agents/")
        || rel.contains("/prompts/")
        || (rel.starts_with("crates/") && rel.ends_with("/src"))
        || rel.contains("/src/") && rel.starts_with("crates/")
}

fn verification_for_path(rel: &str) -> Vec<String> {
    if rel == "." || rel.starts_with("crates/") || rel.starts_with("bin") {
        vec!["cargo test -p aoc-cli dox".to_string()]
    } else if rel.starts_with(".omp/extensions") {
        vec!["bun --check .omp/extensions/aoc-dox.ts".to_string()]
    } else if rel.starts_with("scripts") || rel.ends_with("package.json") {
        vec!["bun test".to_string()]
    } else {
        Vec::new()
    }
}

fn is_public_surface(path: &str) -> bool {
    path.starts_with("bin") || path.starts_with("crates/") || path.contains("/src") || path.starts_with(".omp/extensions")
}

fn is_dynamic_surface(path: &str) -> bool {
    path.contains("extensions") || path.contains("agents") || path.contains("prompts") || path.contains("dispatch")
}

fn is_obvious_layout_rule(path: &str) -> bool {
    matches!(path, "docs" | "tests" | "src")
}

fn build_coverage(
    project_root: &Path,
    directories: &[PathBuf],
    candidates: &[DoxCandidate],
    args: &MapArgs,
) -> Result<Vec<DoxDirectoryCoverage>> {
    let mut coverage = Vec::new();
    for dir in directories {
        let chain = find_agents_chain(project_root, dir)?;
        let chain_rel: Vec<String> = chain.iter().map(|path| rel_path(project_root, path)).collect();
        let bytes = measure_agents_bytes(&chain)?;
        let rel = rel_path(project_root, dir);
        let exact = chain.iter().any(|path| path.parent() == Some(dir));
        let risk_score = candidates.iter().find(|candidate| candidate.path == rel).map(|candidate| candidate.score).unwrap_or(0);
        let mut level = if exact {
            CoverageLevel::Specific
        } else if chain.len() <= 1 {
            CoverageLevel::RootOnly
        } else {
            CoverageLevel::Inherited
        };
        if !exact && risk_score >= args.min_score {
            level = CoverageLevel::Insufficient;
        }
        let status = if bytes > args.active_chain_hard_bytes as u64 {
            CoverageStatus::OverBudget
        } else if level == CoverageLevel::Insufficient {
            CoverageStatus::CandidateLocalAgents
        } else {
            CoverageStatus::Ok
        };
        coverage.push(DoxDirectoryCoverage {
            path: rel.clone(),
            kind: classify_dir(&rel),
            resolved_agents_chain: chain_rel,
            effective_agents_bytes: bytes,
            coverage: level,
            status,
            candidate_path: candidates.iter().find(|candidate| candidate.path == rel).map(|candidate| candidate.path.clone()),
            missing_contracts: Vec::new(),
        });
    }
    coverage.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(coverage)
}

fn mark_coverage_candidates(coverage: &[DoxDirectoryCoverage], candidates: &mut [DoxCandidate]) {
    let insufficient: BTreeSet<&str> = coverage
        .iter()
        .filter(|item| item.coverage == CoverageLevel::Insufficient)
        .map(|item| item.path.as_str())
        .collect();
    for candidate in candidates {
        if insufficient.contains(candidate.path.as_str()) && candidate.reason == "below min_score" {
            candidate.reason = "requires OMP mapper contract extraction".to_string();
        }
    }
}

fn classify_dir(rel: &str) -> DirectoryKind {
    let rel = rel.trim_start_matches("./");
    if rel == "." || rel.starts_with("crates/") && !rel.contains("/src") {
        DirectoryKind::Package
    } else if rel.starts_with("apps/") {
        DirectoryKind::App
    } else if rel.ends_with("src") || rel.contains("/src/") {
        DirectoryKind::Source
    } else if rel.contains("test") || rel.contains("spec") {
        DirectoryKind::Tests
    } else if rel.starts_with("docs") || rel.contains("/docs") {
        DirectoryKind::Docs
    } else if rel.starts_with("scripts") || rel.starts_with("bin") {
        DirectoryKind::Scripts
    } else if rel.contains("generated") {
        DirectoryKind::Generated
    } else if rel.contains("cache") || rel.contains("target") {
        DirectoryKind::Cache
    } else if rel.starts_with('.') {
        DirectoryKind::Config
    } else {
        DirectoryKind::Other
    }
}

fn build_budgets(project_root: &Path, target: u32, hard: u32) -> Result<DoxBudgets> {
    // v1 intentionally supports only AGENTS.md / AGENTS.override.md. Project doc fallback filenames
    // are recorded as unsupported so later support must be explicit.
    let root_agents = project_root.join(ROOT_AGENTS);
    let measured_root_agents_bytes = if root_agents.exists() { fs::metadata(&root_agents)?.len() } else { 0 };
    let cwd = std::env::current_dir().context("resolve current dir")?;
    let chain = find_agents_chain(project_root, &cwd)?;
    let measured_project_chain_bytes = measure_agents_bytes(&chain)?;
    Ok(DoxBudgets {
        root_agents_target_bytes: ROOT_AGENTS_TARGET_BYTES,
        root_agents_hard_bytes: ROOT_AGENTS_HARD_BYTES,
        child_agents_target_bytes: CHILD_AGENTS_TARGET_BYTES,
        child_agents_hard_bytes: CHILD_AGENTS_HARD_BYTES,
        active_chain_target_bytes: target,
        active_chain_hard_bytes: hard,
        measured_root_agents_bytes,
        measured_project_chain_bytes,
        fallback_filenames_supported: false,
        status: budget_status(measured_project_chain_bytes, target, hard),
    })
}

fn budget_status(bytes: u64, target: u32, hard: u32) -> BudgetStatus {
    if bytes > hard as u64 {
        BudgetStatus::OverHard
    } else if bytes > target as u64 {
        BudgetStatus::OverTarget
    } else {
        BudgetStatus::Ok
    }
}

fn build_routes(candidates: &[DoxCandidate]) -> Vec<DoxRoute> {
    candidates
        .iter()
        .filter(|candidate| matches!(candidate.decision, CandidateDecision::Create | CandidateDecision::Update))
        .map(|candidate| DoxRoute {
            path_glob: if candidate.path == "." { "**/*".to_string() } else { format!("{}/**/*", candidate.path) },
            agent_profile: "dox-writer".to_string(),
            required_context: vec![candidate.path.clone()],
            verification: candidate.verification.clone(),
            source_candidate_path: candidate.path.clone(),
        })
        .collect()
}

fn render_report(
    map: &DoxEnvelope<DoxMapData>,
    candidates: &DoxEnvelope<DoxCandidatesData>,
    budgets: &DoxEnvelope<DoxBudgets>,
) -> Result<String> {
    let create = candidates.data.candidates.iter().filter(|candidate| candidate.decision == CandidateDecision::Create).count();
    let update = candidates.data.candidates.iter().filter(|candidate| candidate.decision == CandidateDecision::Update).count();
    let reject = candidates.data.candidates.iter().filter(|candidate| candidate.decision == CandidateDecision::Reject).count();
    Ok(format!(
        "# AOC DOX Report\n\n- Schema: `{}`\n- Directories scanned: {}\n- CodeGraph available: {}\n- Budget status: `{:?}`\n- Candidates: create={}, update={}, reject={}\n\nLocal `AGENTS.md` files are not written by `aoc dox map`; use `aoc dox apply --dry-run` before any apply.\n",
        map.schema,
        map.data.directories.len(),
        map.data.codegraph.available,
        budgets.data.status,
        create,
        update,
        reject
    ))
}

fn render_review_packet(
    map: &DoxEnvelope<DoxMapData>,
    candidates: &DoxEnvelope<DoxCandidatesData>,
    budgets: &DoxEnvelope<DoxBudgets>,
    routes: &DoxEnvelope<DoxRoutesData>,
) -> Result<String> {
    let mut approved: Vec<&DoxCandidate> = candidates
        .data
        .candidates
        .iter()
        .filter(|candidate| matches!(candidate.decision, CandidateDecision::Create | CandidateDecision::Update))
        .collect();
    approved.sort_by_key(|candidate| candidate.target_agents_path.as_deref().unwrap_or(&candidate.path));

    let mut rejected: Vec<&DoxCandidate> = candidates
        .data
        .candidates
        .iter()
        .filter(|candidate| candidate.decision == CandidateDecision::Reject)
        .filter(|candidate| candidate.score >= 7 || !candidate.reason.trim().is_empty())
        .collect();
    rejected.sort_by_key(|candidate| candidate.path.as_str());

    let candidate_routes = build_routes(&candidates.data.candidates);
    let mut rendered = Vec::with_capacity(approved.len());
    for candidate in &approved {
        let target = candidate
            .target_agents_path
            .as_ref()
            .ok_or_else(|| anyhow!("approved candidate missing target_agents_path: {}", candidate.path))?;
        rendered.push((*candidate, target, render_agents_file(candidate)?));
    }

    let mut lines = vec![
        "# AOC DOX Review Packet".to_string(),
        "".to_string(),
        "## Summary".to_string(),
        format!("- Schema: `{}`", SCHEMA_VERSION),
        format!("- Directories scanned: {}", map.data.directories.len()),
        format!("- Budget status: `{:?}`", budgets.data.status),
        format!("- Proposed local AGENTS.md files: {}", rendered.len()),
        format!("- Rejected candidates listed: {}", rejected.len()),
        format!("- Persisted routes: {}", routes.data.routes.len()),
        format!("- Candidate-derived routes: {}", candidate_routes.len()),
        "".to_string(),
        "## Proposed AGENTS.md routes".to_string(),
        "| Target AGENTS.md | Scope | Purpose | Decision | Score | Bytes |".to_string(),
        "|---|---|---|---|---:|---:|".to_string(),
    ];

    for (candidate, target, content) in &rendered {
        push_markdown_table_row(
            &mut lines,
            &[
                target.to_string(),
                candidate.path.clone(),
                candidate_purpose(candidate),
                format!("{:?}", candidate.decision),
                candidate.score.to_string(),
                content.len().to_string(),
            ],
        );
    }

    lines.extend([
        "".to_string(),
        "## Rejected routes".to_string(),
        "| Path | Score | Reason |".to_string(),
        "|---|---:|---|".to_string(),
    ]);
    if rejected.is_empty() {
        lines.push("_None listed._".to_string());
    } else {
        for candidate in &rejected {
            push_markdown_table_row(&mut lines, &[candidate.path.clone(), candidate.score.to_string(), candidate.reason.clone()]);
        }
    }

    lines.extend(["".to_string(), "## Rendered local contracts".to_string()]);
    for (candidate, target, content) in &rendered {
        lines.extend([
            format!("### `{}`", target),
            format!("Scope: `{}`", candidate.path),
            format!("Purpose: {}", candidate_purpose(candidate)),
            "".to_string(),
            "```md".to_string(),
            content.clone(),
            "```".to_string(),
            "".to_string(),
            "Evidence:".to_string(),
        ]);
        if candidate.evidence.is_empty() {
            lines.push("- _None recorded._".to_string());
        } else {
            for evidence in &candidate.evidence {
                let mut parts = Vec::new();
                if let Some(symbol) = evidence.symbol.as_deref().filter(|value| !value.trim().is_empty()) {
                    parts.push(format!("symbol={}", symbol.trim()));
                }
                if let Some(command) = evidence.command.as_deref().filter(|value| !value.trim().is_empty()) {
                    parts.push(format!("command={}", command.trim()));
                }
                if let Some(note) = evidence.note.as_deref().filter(|value| !value.trim().is_empty()) {
                    parts.push(note.trim().replace('\n', " "));
                }
                let suffix = if parts.is_empty() { String::new() } else { format!(" — {}", parts.join("; ")) };
                lines.push(format!("- `{}`{}", evidence.path, suffix));
            }
        }
        lines.extend(["".to_string(), "Verification:".to_string()]);
        if candidate.verification.is_empty() {
            lines.push("- _None recorded._".to_string());
        } else {
            for command in &candidate.verification {
                lines.push(format!("- `{}`", command));
            }
        }
        lines.push("".to_string());
    }

    lines.extend([
        "## Operator apply command".to_string(),
        "After reviewing this packet, apply manually with:".to_string(),
        "".to_string(),
        "```bash".to_string(),
        "aoc dox apply --yes".to_string(),
        "```".to_string(),
    ]);

    Ok(lines.join("\n"))
}

fn candidate_purpose(candidate: &DoxCandidate) -> String {
    fn first_line_capped(value: &str) -> String {
        value.trim().lines().next().unwrap_or("").chars().take(180).collect::<String>()
    }

    let reason = first_line_capped(&candidate.reason);
    if !reason.is_empty() {
        return reason;
    }
    if let Some(rule) = candidate.contracts.iter().map(|contract| first_line_capped(&contract.rule)).find(|rule| !rule.is_empty()) {
        return rule;
    }
    "No purpose recorded; inspect candidate evidence before applying.".to_string()
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn push_markdown_table_row(lines: &mut Vec<String>, cells: &[String]) {
    let row = cells.iter().map(|cell| markdown_cell(cell)).collect::<Vec<_>>().join(" | ");

    lines.push(format!("| {} |", row));
}
fn find_agents_chain(project_root: &Path, cwd: &Path) -> Result<Vec<PathBuf>> {
    let root = project_root.canonicalize().unwrap_or_else(|_| project_root.to_path_buf());
    let cwd_abs = if cwd.exists() { cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf()) } else { cwd.to_path_buf() };
    if !cwd_abs.starts_with(&root) {
        bail!("cwd is outside project root: {}", cwd.display());
    }
    let mut dirs = Vec::new();
    let mut current = root.clone();
    dirs.push(current.clone());
    if cwd_abs != root {
        for component in cwd_abs.strip_prefix(&root)?.components() {
            current.push(component.as_os_str());
            dirs.push(current.clone());
        }
    }
    let mut chain = Vec::new();
    for dir in dirs {
        let override_path = dir.join("AGENTS.override.md");
        let agents_path = dir.join(ROOT_AGENTS);
        if non_empty_file(&override_path) {
            chain.push(override_path);
        } else if non_empty_file(&agents_path) {
            chain.push(agents_path);
        }
    }
    Ok(chain)
}

fn non_empty_file(path: &Path) -> bool {
    fs::metadata(path).map(|metadata| metadata.is_file() && metadata.len() > 0).unwrap_or(false)
}

fn measure_agents_bytes(paths: &[PathBuf]) -> Result<u64> {
    let mut total = 0;
    for path in paths {
        total += fs::metadata(path).with_context(|| format!("stat {}", path.display()))?.len();
    }
    Ok(total)
}

fn render_agents_file(candidate: &DoxCandidate) -> Result<String> {
    if candidate.contracts.is_empty() {
        bail!("candidate has no local contracts: {}", candidate.path);
    }
    let mut lines = vec![
        "# Repository Guidelines".to_string(),
        "".to_string(),
        format!("Scope: `{}`", candidate.path),
        "".to_string(),
        "## Local Contracts".to_string(),
    ];
    for contract in &candidate.contracts {
        lines.push(format!("- {}", contract.rule));
    }
    let verification = collect_contract_values(candidate, |contract| &contract.verification);
    if !verification.is_empty() {
        lines.push("".to_string());
        lines.push("## Verification".to_string());
        for command in verification {
            lines.push(format!("- `{}`", command));
        }
    }
    let do_not = collect_contract_values(candidate, |contract| &contract.do_not);
    if !do_not.is_empty() {
        lines.push("".to_string());
        lines.push("## Do Not".to_string());
        for item in do_not {
            lines.push(format!("- {}", item));
        }
    }
    let update_when = collect_contract_values(candidate, |contract| &contract.update_when);
    if !update_when.is_empty() {
        lines.push("".to_string());
        lines.push("## Update When".to_string());
        for item in update_when {
            lines.push(format!("- {}", item));
        }
    }
    lines.push("".to_string());
    Ok(lines.join("\n"))
}

fn collect_contract_values<F>(candidate: &DoxCandidate, accessor: F) -> Vec<String>
where
    F: Fn(&LocalContract) -> &Vec<String>,
{
    let mut values = BTreeSet::new();
    for contract in &candidate.contracts {
        for item in accessor(contract) {
            if !item.trim().is_empty() {
                values.insert(item.trim().to_string());
            }
        }
    }
    values.into_iter().collect()
}

fn validate_schema(schema: &str, label: &str, errors: &mut Vec<String>) {
    if schema != SCHEMA_VERSION {
        errors.push(format!("{} has unsupported schema: {}", label, schema));
    }
}

fn validate_candidates(project_root: &Path, candidates: &[DoxCandidate], errors: &mut Vec<String>) {
    for candidate in candidates {
        if matches!(candidate.decision, CandidateDecision::Create | CandidateDecision::Update) {
            if candidate.evidence.is_empty() {
                errors.push(format!("candidate missing evidence: {}", candidate.path));
            }
            if candidate.verification.is_empty() {
                errors.push(format!("candidate missing verification: {}", candidate.path));
            }
            for evidence in &candidate.evidence {
                if evidence.path.is_empty() && evidence.command.is_some() {
                    continue;
                }
                if !project_root.join(&evidence.path).exists() {
                    errors.push(format!("candidate evidence path missing: {}", evidence.path));
                }
            }
        }
    }
}

fn all_metadata_commands(
    map: &DoxEnvelope<DoxMapData>,
    candidates: &DoxEnvelope<DoxCandidatesData>,
    routes: &DoxEnvelope<DoxRoutesData>,
) -> Vec<String> {
    let mut commands = Vec::new();
    commands.extend(map.data.codegraph.commands.iter().map(|command| command.command.clone()));
    for candidate in &candidates.data.candidates {
        commands.extend(candidate.verification.clone());
        for evidence in &candidate.evidence {
            if let Some(command) = &evidence.command {
                commands.push(command.clone());
            }
        }
    }
    for route in &routes.data.routes {
        commands.extend(route.verification.clone());
    }
    commands
}

fn validate_verification_command(command: &str) -> Result<()> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        bail!("verification command is empty");
    }
    for token in [
        " rm ",
        " rm -",
        "git push",
        "jj git push",
        "npm install",
        "pnpm install",
        "bun install",
        "cargo publish",
        "vercel --prod",
        "convex deploy",
    ] {
        if format!(" {} ", trimmed).contains(token) || trimmed.starts_with(token.trim_start()) {
            bail!("destructive verification command is not allowed: {}", command);
        }
    }
    Ok(())
}

fn rel_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .ok()
        .and_then(|rel| {
            let value = rel.to_string_lossy().replace('\\', "/");
            if value.is_empty() { Some(".".to_string()) } else { Some(value) }
        })
        .unwrap_or_else(|| path.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = Utc::now().timestamp_nanos_opt().unwrap_or_default();
        path.push(format!("aoc-dox-{}-{}-{}", name, std::process::id(), nanos));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn evidence(path: &str) -> EvidenceRef {
        EvidenceRef { path: path.to_string(), symbol: None, command: None, note: None }
    }

    fn sample_candidate(path: &str, target: &str, decision: CandidateDecision) -> DoxCandidate {
        DoxCandidate {
            path: path.to_string(),
            decision,
            score: 8,
            confidence: 0.8,
            reason: "High-risk local conventions need local context.".to_string(),
            contracts: vec![LocalContract {
                rule: "Keep DOX local contracts evidence-backed.".to_string(),
                do_not: vec![],
                update_when: vec!["DOX metadata schema changes.".to_string()],
                verification: vec!["cargo test -p aoc-cli dox".to_string()],
                evidence: vec![evidence("crates/aoc-cli/src/dox.rs")],
            }],
            risks: vec![],
            verification: vec!["cargo test -p aoc-cli dox".to_string()],
            evidence: vec![EvidenceRef {
                path: "crates/aoc-cli/src/dox.rs".to_string(),
                symbol: Some("render_review_packet".to_string()),
                command: Some("cargo test -p aoc-cli dox".to_string()),
                note: Some("packet renderer coverage".to_string()),
            }],
            target_agents_path: Some(target.to_string()),
        }
    }

    fn test_envelope<T>(data: T) -> DoxEnvelope<T> {
        DoxEnvelope {
            schema: SCHEMA_VERSION.to_string(),
            generated_at: "2026-06-13T00:00:00Z".to_string(),
            project_root: "/tmp/aoc".to_string(),
            data,
        }
    }

    fn empty_map() -> DoxEnvelope<DoxMapData> {
        test_envelope(DoxMapData {
            codegraph: CodeGraphSummary { available: false, disabled: true, commands: vec![], errors_log: None },
            directories: vec!["crates/aoc-cli".to_string()],
            package_manifests: vec![],
            instruction_files: vec![],
            test_configs: vec![],
            generated_markers: vec![],
            coverage: vec![],
        })
    }

    fn test_budgets() -> DoxEnvelope<DoxBudgets> {
        test_envelope(DoxBudgets {
            root_agents_target_bytes: ROOT_AGENTS_TARGET_BYTES,
            root_agents_hard_bytes: ROOT_AGENTS_HARD_BYTES,
            child_agents_target_bytes: CHILD_AGENTS_TARGET_BYTES,
            child_agents_hard_bytes: CHILD_AGENTS_HARD_BYTES,
            active_chain_target_bytes: 16_384,
            active_chain_hard_bytes: 24_576,
            measured_root_agents_bytes: 0,
            measured_project_chain_bytes: 0,
            fallback_filenames_supported: false,
            status: BudgetStatus::Ok,
        })
    }

    fn empty_routes() -> DoxEnvelope<DoxRoutesData> {
        test_envelope(DoxRoutesData { routes: vec![] })
    }

    #[test]
    fn score_rejects_candidate_without_verification_even_when_high_risk() {
        let decision = score_candidate(
            ".omp/extensions",
            true,
            true,
            &["high-risk invariant".to_string()],
            &[],
            &[evidence("AGENTS.md")],
            7,
        );
        assert_eq!(decision.decision, CandidateDecision::Reject);
        assert_eq!(decision.reason, "missing evidence or verification");
    }

    #[test]
    fn score_accepts_cli_surface_with_evidence_and_verification() {
        let decision = score_candidate(
            "crates/aoc-cli/src/dox.rs",
            true,
            true,
            &["high-risk invariant".to_string()],
            &["cargo test -p aoc-cli dox".to_string()],
            &[evidence("crates/aoc-cli/src/dox.rs")],
            7,
        );
        assert!(decision.score >= 7);
        assert!(matches!(decision.decision, CandidateDecision::Create | CandidateDecision::Update));
    }

    #[test]
    fn agents_chain_prefers_override_and_skips_empty() {
        let root = temp_root("chain");
        fs::write(root.join("AGENTS.md"), "root").unwrap();
        let child = root.join("child");
        fs::create_dir_all(&child).unwrap();
        fs::write(child.join("AGENTS.md"), "child agents").unwrap();
        fs::write(child.join("AGENTS.override.md"), "override").unwrap();
        let empty = child.join("empty");
        fs::create_dir_all(&empty).unwrap();
        fs::write(empty.join("AGENTS.override.md"), "").unwrap();
        let chain = find_agents_chain(&root, &empty).unwrap();
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[1], child.join("AGENTS.override.md"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn coverage_marks_root_only_inherited_specific_and_insufficient() {
        let root = temp_root("coverage");
        fs::write(root.join("AGENTS.md"), "root").unwrap();
        let parent = root.join("parent");
        let inherited = parent.join("inherited");
        let specific = root.join("specific");
        let risky = root.join("scripts");
        fs::create_dir_all(&inherited).unwrap();
        fs::create_dir_all(&specific).unwrap();
        fs::create_dir_all(&risky).unwrap();
        fs::write(parent.join("AGENTS.md"), "parent").unwrap();
        fs::write(specific.join("AGENTS.md"), "specific").unwrap();
        let dirs = vec![root.clone(), inherited.clone(), specific.clone(), risky.clone()];
        let candidates = vec![
            DoxCandidate { path: ".".to_string(), decision: CandidateDecision::Reject, score: 0, confidence: 0.0, reason: String::new(), contracts: vec![], risks: vec![], verification: vec![], evidence: vec![], target_agents_path: None },
            DoxCandidate { path: "parent/inherited".to_string(), decision: CandidateDecision::Reject, score: 0, confidence: 0.0, reason: String::new(), contracts: vec![], risks: vec![], verification: vec![], evidence: vec![], target_agents_path: None },
            DoxCandidate { path: "specific".to_string(), decision: CandidateDecision::Reject, score: 0, confidence: 0.0, reason: String::new(), contracts: vec![], risks: vec![], verification: vec![], evidence: vec![], target_agents_path: None },
            DoxCandidate { path: "scripts".to_string(), decision: CandidateDecision::Reject, score: 7, confidence: 0.0, reason: String::new(), contracts: vec![], risks: vec![], verification: vec![], evidence: vec![], target_agents_path: None },
        ];
        let args = MapArgs { json: false, no_codegraph: true, min_score: 7, max_codegraph_chars: 12000, active_chain_target_bytes: 16384, active_chain_hard_bytes: 24576 };
        let coverage = build_coverage(&root, &dirs, &candidates, &args).unwrap();
        assert_eq!(coverage.iter().find(|item| item.path == ".").unwrap().coverage, CoverageLevel::Specific);
        assert_eq!(coverage.iter().find(|item| item.path == "parent/inherited").unwrap().coverage, CoverageLevel::Inherited);
        assert_eq!(coverage.iter().find(|item| item.path == "specific").unwrap().coverage, CoverageLevel::Specific);
        assert_eq!(coverage.iter().find(|item| item.path == "scripts").unwrap().coverage, CoverageLevel::Insufficient);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn budget_status_marks_over_hard() {
        assert_eq!(budget_status(25_000, 16_384, 24_576), BudgetStatus::OverHard);
    }

    #[test]
    fn render_agents_file_omits_empty_optional_sections() {
        let candidate = DoxCandidate {
            path: "crates/aoc-cli".to_string(),
            decision: CandidateDecision::Create,
            score: 7,
            confidence: 0.8,
            reason: String::new(),
            contracts: vec![LocalContract { rule: "Keep CLI flags stable.".to_string(), do_not: vec![], update_when: vec![], verification: vec![], evidence: vec![evidence("AGENTS.md")] }],
            risks: vec![],
            verification: vec![],
            evidence: vec![evidence("AGENTS.md")],
            target_agents_path: Some("crates/aoc-cli/AGENTS.md".to_string()),
        };
        let output = render_agents_file(&candidate).unwrap();
        assert!(output.contains("# Repository Guidelines"));
        assert!(output.contains("Scope: `crates/aoc-cli`"));
        assert!(output.contains("## Local Contracts"));
        assert!(!output.contains("## Verification"));
        assert!(!output.contains("## Do Not"));
        assert!(!output.contains("## Update When"));
    }


    #[test]
    fn review_packet_lists_routes_rejects_content_and_apply_command() {
        let create = sample_candidate("crates/aoc-cli", "crates/aoc-cli/AGENTS.md", CandidateDecision::Create);
        let reject = DoxCandidate {
            path: "crates/aoc-control/src".to_string(),
            decision: CandidateDecision::Reject,
            score: 8,
            confidence: 0.7,
            reason: "verification mismatch".to_string(),
            contracts: vec![],
            risks: vec![],
            verification: vec![],
            evidence: vec![],
            target_agents_path: None,
        };
        let candidates = test_envelope(DoxCandidatesData { candidates: vec![create, reject] });
        let output = render_review_packet(&empty_map(), &candidates, &test_budgets(), &empty_routes()).unwrap();

        assert!(output.contains("# AOC DOX Review Packet"));
        assert!(output.contains("crates/aoc-cli/AGENTS.md"));
        assert!(output.contains("Scope: `crates/aoc-cli`"));
        assert!(output.contains("```md\n# Repository Guidelines"));
        assert!(output.contains("crates/aoc-control/src"));
        assert!(output.contains("aoc dox apply --yes"));
    }

    #[test]
    fn markdown_cell_escapes_pipes_and_newlines() {
        assert_eq!(markdown_cell("a|b\nc"), "a\\|b c");
    }

    #[test]
    fn review_packet_rejects_approved_candidate_without_target() {
        let mut candidate = sample_candidate("crates/aoc-cli", "crates/aoc-cli/AGENTS.md", CandidateDecision::Create);
        candidate.target_agents_path = None;
        let candidates = test_envelope(DoxCandidatesData { candidates: vec![candidate] });
        let error = render_review_packet(&empty_map(), &candidates, &test_budgets(), &empty_routes()).unwrap_err();
        assert!(error.to_string().contains("approved candidate missing target_agents_path"));
    }

    #[test]
    fn candidate_purpose_prefers_reason_then_rule() {
        let with_reason = sample_candidate("crates/aoc-cli", "crates/aoc-cli/AGENTS.md", CandidateDecision::Create);
        assert_eq!(candidate_purpose(&with_reason), "High-risk local conventions need local context.");

        let mut with_rule = sample_candidate("crates/aoc-cli", "crates/aoc-cli/AGENTS.md", CandidateDecision::Create);
        with_rule.reason.clear();
        assert_eq!(candidate_purpose(&with_rule), "Keep DOX local contracts evidence-backed.");
    }
    #[test]
    fn doctor_rejects_destructive_verification_command() {
        assert!(validate_verification_command("git push origin main").is_err());
        assert!(validate_verification_command("bun install").is_err());
        assert!(validate_verification_command("cargo test -p aoc-cli dox").is_ok());
    }
}

use aoc_core::insight_contracts::{
    InsightBootstrapGap, InsightBootstrapGapKind, InsightBootstrapRequest, InsightBootstrapResult,
    InsightDispatchMode, InsightDispatchRequest, InsightDispatchResult, InsightDispatchStepResult,
    InsightSeedJob, InsightTaskProposal,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

const AGENTS_DIR: &str = ".pi/agents";
const TEAMS_FILE: &str = ".pi/agents/teams.yaml";
const CHAIN_FILE: &str = ".pi/agents/agent-chain.yaml";
const DEFAULT_DISPATCH_AGENT: &str = "insight-t1-observer";
const DEFAULT_TEAM: &str = "insight-core";
const DEFAULT_CHAIN: &str = "insight-handoff";

#[derive(Clone)]
pub struct InsightSupervisor {
    project_root: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ChainStep {
    agent: String,
    #[allow(dead_code)]
    prompt: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChainDefinition {
    #[allow(dead_code)]
    description: Option<String>,
    #[serde(default)]
    steps: Vec<ChainStep>,
}

impl InsightSupervisor {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }

    pub fn dispatch(&self, request: &InsightDispatchRequest) -> InsightDispatchResult {
        let started = Instant::now();
        let (mode, mut steps) = match request.mode {
            InsightDispatchMode::Dispatch => {
                let agent = request
                    .agent
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(DEFAULT_DISPATCH_AGENT)
                    .to_string();
                let step = self.run_agent(&agent, &request.input);
                (InsightDispatchMode::Dispatch, vec![step])
            }
            InsightDispatchMode::Chain => {
                let chain_name = request
                    .chain
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(DEFAULT_CHAIN);
                (
                    InsightDispatchMode::Chain,
                    self.run_chain(chain_name, &request.input),
                )
            }
            InsightDispatchMode::Parallel => {
                let team_name = request
                    .team
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(DEFAULT_TEAM);
                (
                    InsightDispatchMode::Parallel,
                    self.run_parallel(team_name, &request.input),
                )
            }
        };

        if steps.is_empty() {
            steps.push(InsightDispatchStepResult {
                agent: "insight-supervisor".to_string(),
                status: "fallback".to_string(),
                output_excerpt: None,
                error: Some("no steps resolved from manifests".to_string()),
            });
        }

        let fallback_used = steps.iter().any(|step| step.status != "success");
        let status = if fallback_used { "fallback" } else { "ok" };
        let summary = format!(
            "insight dispatch mode={} steps={} fallback={}",
            mode_label(mode),
            steps.len(),
            fallback_used
        );

        InsightDispatchResult {
            mode,
            status: status.to_string(),
            summary,
            steps,
            fallback_used,
            duration_ms: Some(started.elapsed().as_millis() as u64),
        }
    }

    pub fn bootstrap(&self, request: &InsightBootstrapRequest) -> InsightBootstrapResult {
        let mut docs = BTreeMap::<String, String>::new();
        let mut code = BTreeMap::<String, String>::new();

        for path in self.collect_docs(&request.scope_paths) {
            if let Some(key) = normalized_key(&path) {
                docs.entry(key)
                    .or_insert_with(|| relativize(&self.project_root, &path));
            }
        }

        for path in self.collect_code(&request.scope_paths) {
            if let Some(key) = normalized_key(&path) {
                code.entry(key)
                    .or_insert_with(|| relativize(&self.project_root, &path));
            }
        }

        let mut gaps = Vec::new();

        for (key, doc_ref) in &docs {
            if code.contains_key(key) || is_generic_doc_key(key) {
                continue;
            }
            gaps.push(InsightBootstrapGap {
                gap_id: gap_id("missing", key),
                kind: InsightBootstrapGapKind::MissingImplementation,
                severity: "high".to_string(),
                confidence: "medium".to_string(),
                summary: format!("Documented behavior appears missing in code: {key}"),
                evidence_refs: vec![doc_ref.clone()],
            });
        }

        for (key, code_ref) in &code {
            if docs.contains_key(key) || is_generic_code_key(key) {
                continue;
            }
            gaps.push(InsightBootstrapGap {
                gap_id: gap_id("undoc", key),
                kind: InsightBootstrapGapKind::UndocumentedCode,
                severity: "medium".to_string(),
                confidence: "medium".to_string(),
                summary: format!("Code path lacks matching docs coverage: {key}"),
                evidence_refs: vec![code_ref.clone()],
            });
        }

        gaps.sort_by(|a, b| a.gap_id.cmp(&b.gap_id));
        if gaps.len() > request.max_gaps {
            gaps.truncate(request.max_gaps);
        }

        let taskmaster_projection = gaps
            .iter()
            .take(6)
            .map(|gap| InsightTaskProposal {
                title: format!("Insight gap: {}", gap.summary),
                priority: if gap.severity == "high" {
                    "high".to_string()
                } else {
                    "medium".to_string()
                },
                rationale: format!("Generated by insight_bootstrap dry-run from {}", gap.gap_id),
                evidence_refs: gap.evidence_refs.clone(),
            })
            .collect::<Vec<_>>();

        let scope_tag = request
            .active_tag
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("global")
            .to_string();

        let seeds = gaps
            .iter()
            .filter(|gap| gap.severity == "high")
            .take(4)
            .map(|gap| InsightSeedJob {
                seed_id: format!("seed:{}", gap.gap_id),
                scope_tag: scope_tag.clone(),
                source_gap_ids: vec![gap.gap_id.clone()],
                priority: "high".to_string(),
                reason: format!("High-priority ambiguity from {}", gap.summary),
            })
            .collect::<Vec<_>>();

        InsightBootstrapResult {
            dry_run: request.dry_run,
            gaps,
            taskmaster_projection,
            seeds,
        }
    }

    fn run_chain(&self, chain_name: &str, original_input: &str) -> Vec<InsightDispatchStepResult> {
        let chains = self.load_chain_manifest();
        let Some(chain) = chains.get(chain_name) else {
            return vec![InsightDispatchStepResult {
                agent: "insight-supervisor".to_string(),
                status: "fallback".to_string(),
                output_excerpt: None,
                error: Some(format!("chain not found: {chain_name}")),
            }];
        };

        let mut prior_output = original_input.to_string();
        let mut results = Vec::new();
        for step in &chain.steps {
            let prompt = step
                .prompt
                .as_deref()
                .unwrap_or("$INPUT")
                .replace("$ORIGINAL", original_input)
                .replace("$INPUT", &prior_output);
            let result = self.run_agent(&step.agent, &prompt);
            if let Some(output) = result.output_excerpt.as_ref() {
                prior_output = output.clone();
            }
            results.push(result);
        }

        results
    }

    fn run_parallel(&self, team_name: &str, input: &str) -> Vec<InsightDispatchStepResult> {
        let teams = self.load_team_manifest();
        let Some(agents) = teams.get(team_name) else {
            return vec![InsightDispatchStepResult {
                agent: "insight-supervisor".to_string(),
                status: "fallback".to_string(),
                output_excerpt: None,
                error: Some(format!("team not found: {team_name}")),
            }];
        };

        let mut handles = Vec::new();
        for agent in agents {
            let supervisor = self.clone();
            let agent_name = agent.clone();
            let input_text = input.to_string();
            handles.push(std::thread::spawn(move || {
                supervisor.run_agent(&agent_name, &input_text)
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.join() {
                Ok(result) => results.push(result),
                Err(_) => results.push(InsightDispatchStepResult {
                    agent: "insight-supervisor".to_string(),
                    status: "fallback".to_string(),
                    output_excerpt: None,
                    error: Some("parallel worker panicked".to_string()),
                }),
            }
        }
        results
    }

    fn run_agent(&self, agent: &str, input: &str) -> InsightDispatchStepResult {
        let agent_file = self
            .project_root
            .join(AGENTS_DIR)
            .join(format!("{agent}.md"));
        if !agent_file.exists() {
            return InsightDispatchStepResult {
                agent: agent.to_string(),
                status: "fallback".to_string(),
                output_excerpt: None,
                error: Some(format!(
                    "agent definition missing: {}",
                    relativize(&self.project_root, &agent_file)
                )),
            };
        }

        let Some(cmdline) = resolve_agent_command() else {
            return InsightDispatchStepResult {
                agent: agent.to_string(),
                status: "fallback".to_string(),
                output_excerpt: Some(truncate_chars(
                    format!(
                        "deterministic fallback: set AOC_INSIGHT_AGENT_CMD to enable subprocess execution for {agent}"
                    ),
                    280,
                )),
                error: Some("insight subprocess command not configured".to_string()),
            };
        };

        let mut command = Command::new("bash");
        command
            .arg("-lc")
            .arg(cmdline)
            .current_dir(&self.project_root)
            .env("AOC_INSIGHT_AGENT", agent)
            .env("AOC_INSIGHT_INPUT", input)
            .env(
                "AOC_INSIGHT_AGENT_FILE",
                relativize(&self.project_root, &agent_file),
            );

        match command.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if output.status.success() {
                    InsightDispatchStepResult {
                        agent: agent.to_string(),
                        status: "success".to_string(),
                        output_excerpt: Some(truncate_chars(stdout, 400)),
                        error: None,
                    }
                } else {
                    InsightDispatchStepResult {
                        agent: agent.to_string(),
                        status: "fallback".to_string(),
                        output_excerpt: Some(truncate_chars(stdout, 280)),
                        error: Some(truncate_chars(
                            if stderr.is_empty() {
                                format!("agent exited with status {}", output.status)
                            } else {
                                stderr
                            },
                            320,
                        )),
                    }
                }
            }
            Err(err) => InsightDispatchStepResult {
                agent: agent.to_string(),
                status: "fallback".to_string(),
                output_excerpt: None,
                error: Some(format!("failed to spawn agent subprocess: {err}")),
            },
        }
    }

    fn load_team_manifest(&self) -> BTreeMap<String, Vec<String>> {
        let path = self.project_root.join(TEAMS_FILE);
        let Some(contents) = read_text(&path) else {
            return BTreeMap::new();
        };
        let parsed = serde_yaml::from_str::<BTreeMap<String, Vec<String>>>(&contents);
        let Ok(mut teams) = parsed else {
            return BTreeMap::new();
        };
        for members in teams.values_mut() {
            members.retain(|member| !member.trim().is_empty());
        }
        teams.retain(|_, members| !members.is_empty());
        teams
    }

    fn load_chain_manifest(&self) -> BTreeMap<String, ChainDefinition> {
        let path = self.project_root.join(CHAIN_FILE);
        let Some(contents) = read_text(&path) else {
            return BTreeMap::new();
        };
        let parsed = serde_yaml::from_str::<BTreeMap<String, ChainDefinition>>(&contents);
        let Ok(mut chains) = parsed else {
            return BTreeMap::new();
        };
        chains.retain(|_, def| !def.steps.is_empty());
        chains
    }

    fn collect_docs(&self, scope_paths: &[String]) -> Vec<PathBuf> {
        self.collect_files(scope_paths, |path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("md"))
                .unwrap_or(false)
        })
    }

    fn collect_code(&self, scope_paths: &[String]) -> Vec<PathBuf> {
        self.collect_files(scope_paths, |path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| {
                    matches!(
                        ext.to_ascii_lowercase().as_str(),
                        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "sh"
                    )
                })
                .unwrap_or(false)
        })
    }

    fn collect_files<F>(&self, scope_paths: &[String], accept: F) -> Vec<PathBuf>
    where
        F: Fn(&Path) -> bool,
    {
        let mut roots = BTreeSet::<PathBuf>::new();
        if scope_paths.is_empty() {
            roots.insert(self.project_root.join("docs"));
            roots.insert(self.project_root.join("crates"));
            roots.insert(self.project_root.join(".pi"));
            roots.insert(self.project_root.join("bin"));
        } else {
            for scope in scope_paths {
                let trimmed = scope.trim();
                if trimmed.is_empty() {
                    continue;
                }
                roots.insert(self.project_root.join(trimmed));
            }
        }

        let mut files = Vec::new();
        for root in roots {
            if !root.exists() {
                continue;
            }
            walk_collect(&root, &accept, &mut files);
        }

        files
    }
}

fn walk_collect<F>(path: &Path, accept: &F, out: &mut Vec<PathBuf>)
where
    F: Fn(&Path) -> bool,
{
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_file() {
        if accept(path) {
            out.push(path.to_path_buf());
        }
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        let child = entry.path();
        let name = child
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if name == ".git" || name == "target" || name == "node_modules" {
            continue;
        }
        walk_collect(&child, accept, out);
    }
}

fn resolve_agent_command() -> Option<String> {
    std::env::var("AOC_INSIGHT_AGENT_CMD")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_text(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn mode_label(mode: InsightDispatchMode) -> &'static str {
    match mode {
        InsightDispatchMode::Dispatch => "dispatch",
        InsightDispatchMode::Chain => "chain",
        InsightDispatchMode::Parallel => "parallel",
    }
}

fn normalized_key(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let lower = stem.to_ascii_lowercase();
    let key = lower
        .replace("_prd_rpg", "")
        .replace("_prd", "")
        .replace("-prd", "")
        .replace("_spec", "")
        .replace("-spec", "")
        .replace('_', "-")
        .replace('.', "-");
    let key = key.trim_matches('-').to_string();
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

fn is_generic_doc_key(key: &str) -> bool {
    matches!(
        key,
        "readme" | "overview" | "index" | "installation" | "configuration" | "agents" | "changelog"
    )
}

fn is_generic_code_key(key: &str) -> bool {
    matches!(key, "main" | "lib" | "mod")
}

fn relativize(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

fn gap_id(prefix: &str, key: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    prefix.hash(&mut hasher);
    key.hash(&mut hasher);
    format!("{prefix}:{:08x}", hasher.finish() as u32)
}

fn truncate_chars(text: String, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text;
    }
    let mut out = text
        .chars()
        .take(limit.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn fixture_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "aoc-insight-orch-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(root.join(".pi/agents")).expect("agents");
        fs::create_dir_all(root.join("docs")).expect("docs");
        fs::create_dir_all(root.join("crates/aoc-sample/src")).expect("src");

        fs::write(
            root.join(".pi/agents/insight-t1-observer.md"),
            "---\nname: insight-t1-observer\n---\n",
        )
        .expect("agent");
        fs::write(
            root.join(".pi/agents/insight-t2-reflector.md"),
            "---\nname: insight-t2-reflector\n---\n",
        )
        .expect("agent");
        fs::write(
            root.join(".pi/agents/teams.yaml"),
            "insight-core:\n  - insight-t1-observer\n  - insight-t2-reflector\n",
        )
        .expect("teams");
        fs::write(
            root.join(".pi/agents/agent-chain.yaml"),
            "insight-handoff:\n  steps:\n    - agent: insight-t1-observer\n      prompt: \"$INPUT\"\n    - agent: insight-t2-reflector\n      prompt: \"$INPUT\"\n",
        )
        .expect("chain");

        fs::write(root.join("docs/insight-runtime.md"), "# insight runtime").expect("doc");
        fs::write(root.join("crates/aoc-sample/src/runtime.rs"), "fn main(){}").expect("code");

        root
    }

    #[test]
    fn dispatch_without_command_falls_back_deterministically() {
        let _guard = env_lock().lock().expect("env lock");
        let root = fixture_root();
        let old = std::env::var("AOC_INSIGHT_AGENT_CMD").ok();
        std::env::remove_var("AOC_INSIGHT_AGENT_CMD");

        let supervisor = InsightSupervisor::new(&root);
        let result = supervisor.dispatch(&InsightDispatchRequest {
            mode: InsightDispatchMode::Dispatch,
            agent: Some("insight-t1-observer".to_string()),
            input: "summarize".to_string(),
            ..InsightDispatchRequest::default()
        });

        assert_eq!(result.mode, InsightDispatchMode::Dispatch);
        assert_eq!(result.status, "fallback");
        assert_eq!(result.steps.len(), 1);

        if let Some(previous) = old {
            std::env::set_var("AOC_INSIGHT_AGENT_CMD", previous);
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn bootstrap_produces_gap_projection() {
        let root = fixture_root();
        let supervisor = InsightSupervisor::new(&root);

        let result = supervisor.bootstrap(&InsightBootstrapRequest::default());
        assert!(result.dry_run);
        assert!(!result.gaps.is_empty());
        assert!(!result.taskmaster_projection.is_empty());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dispatch_executes_configured_subprocess() {
        let _guard = env_lock().lock().expect("env lock");
        let root = fixture_root();
        let script_path = root.join("emit-insight.sh");
        let mut script = fs::File::create(&script_path).expect("script");
        writeln!(
            script,
            "#!/usr/bin/env bash\necho \"agent=$AOC_INSIGHT_AGENT\"\necho \"input=$AOC_INSIGHT_INPUT\""
        )
        .expect("write script");
        script.flush().expect("flush script");
        drop(script);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path).expect("meta").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).expect("chmod");
        }

        let old = std::env::var("AOC_INSIGHT_AGENT_CMD").ok();
        std::env::set_var(
            "AOC_INSIGHT_AGENT_CMD",
            format!("bash '{}'", script_path.to_string_lossy()),
        );

        let supervisor = InsightSupervisor::new(&root);
        let result = supervisor.dispatch(&InsightDispatchRequest {
            mode: InsightDispatchMode::Dispatch,
            agent: Some("insight-t1-observer".to_string()),
            input: "hello".to_string(),
            ..InsightDispatchRequest::default()
        });

        assert_eq!(result.status, "ok");
        assert_eq!(result.steps[0].status, "success");
        assert!(result.steps[0]
            .output_excerpt
            .as_deref()
            .unwrap_or_default()
            .contains("agent=insight-t1-observer"));

        if let Some(previous) = old {
            std::env::set_var("AOC_INSIGHT_AGENT_CMD", previous);
        } else {
            std::env::remove_var("AOC_INSIGHT_AGENT_CMD");
        }
        let _ = fs::remove_dir_all(root);
    }
}

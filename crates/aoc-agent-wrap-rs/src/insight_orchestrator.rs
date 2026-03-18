use aoc_core::insight_contracts::{
    InsightBootstrapGap, InsightBootstrapGapKind, InsightBootstrapRequest, InsightBootstrapResult,
    InsightDetachedCancelRequest, InsightDetachedCancelResult, InsightDetachedDispatchRequest,
    InsightDetachedDispatchResult, InsightDetachedJob, InsightDetachedJobStatus,
    InsightDetachedMode, InsightDetachedStatusRequest, InsightDetachedStatusResult,
    InsightDispatchMode, InsightDispatchRequest, InsightDispatchResult, InsightDispatchStepResult,
    InsightSeedJob, InsightTaskProposal,
};
use aoc_storage::MindStore;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

const AGENTS_DIR: &str = ".pi/agents";
const TEAMS_FILE: &str = ".pi/agents/teams.yaml";
const CHAIN_FILE: &str = ".pi/agents/agent-chain.yaml";
const DEFAULT_DISPATCH_AGENT: &str = "insight-t1-observer";
const DEFAULT_TEAM: &str = "insight-core";
const DEFAULT_CHAIN: &str = "insight-handoff";
const DEFAULT_PARALLEL_CONCURRENCY: usize = 2;

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
                stdout_excerpt: None,
                stderr_excerpt: None,
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
                stdout_excerpt: None,
                stderr_excerpt: None,
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
                stdout_excerpt: None,
                stderr_excerpt: None,
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
                    stdout_excerpt: None,
                    stderr_excerpt: None,
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
                stdout_excerpt: None,
                stderr_excerpt: None,
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
                stdout_excerpt: None,
                stderr_excerpt: None,
                error: Some("insight subprocess command not configured".to_string()),
            };
        };

        let mut command = Command::new("bash");
        super::configure_mind_child_std_command_env(
            &mut command,
            vec![
                ("AOC_INSIGHT_AGENT".to_string(), agent.to_string()),
                ("AOC_INSIGHT_INPUT".to_string(), input.to_string()),
                (
                    "AOC_INSIGHT_AGENT_FILE".to_string(),
                    relativize(&self.project_root, &agent_file),
                ),
            ],
        );
        command
            .arg("-lc")
            .arg(cmdline)
            .current_dir(&self.project_root);

        match command.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if output.status.success() {
                    InsightDispatchStepResult {
                        agent: agent.to_string(),
                        status: "success".to_string(),
                        output_excerpt: Some(truncate_chars(stdout.clone(), 400)),
                        stdout_excerpt: Some(truncate_chars(stdout, 400)).filter(|v| !v.is_empty()),
                        stderr_excerpt: Some(truncate_chars(stderr, 240)).filter(|v| !v.is_empty()),
                        error: None,
                    }
                } else {
                    InsightDispatchStepResult {
                        agent: agent.to_string(),
                        status: "fallback".to_string(),
                        output_excerpt: Some(truncate_chars(stdout.clone(), 280)),
                        stdout_excerpt: Some(truncate_chars(stdout, 280)).filter(|v| !v.is_empty()),
                        stderr_excerpt: Some(truncate_chars(stderr.clone(), 240))
                            .filter(|v| !v.is_empty()),
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
                stdout_excerpt: None,
                stderr_excerpt: None,
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

#[derive(Clone)]
pub struct DetachedInsightRuntime {
    supervisor: InsightSupervisor,
    store_path: PathBuf,
    jobs: Arc<Mutex<BTreeMap<String, InsightDetachedJob>>>,
    running_pids: Arc<Mutex<BTreeMap<String, BTreeSet<u32>>>>,
    cancelled_jobs: Arc<Mutex<BTreeSet<String>>>,
    next_job_id: Arc<AtomicU64>,
}

impl DetachedInsightRuntime {
    pub fn new(project_root: impl Into<PathBuf>, store_path: impl Into<PathBuf>) -> Self {
        let project_root = project_root.into();
        let store_path = store_path.into();
        let recovered_jobs = recover_persisted_jobs(&store_path);
        let next_job_id = recovered_jobs
            .iter()
            .filter_map(|job| {
                job.job_id
                    .rsplit_once('-')
                    .and_then(|(_, suffix)| suffix.parse::<u64>().ok())
            })
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        Self {
            supervisor: InsightSupervisor::new(project_root),
            store_path,
            jobs: Arc::new(Mutex::new(
                recovered_jobs
                    .into_iter()
                    .map(|job| (job.job_id.clone(), job))
                    .collect(),
            )),
            running_pids: Arc::new(Mutex::new(BTreeMap::new())),
            cancelled_jobs: Arc::new(Mutex::new(BTreeSet::new())),
            next_job_id: Arc::new(AtomicU64::new(next_job_id)),
        }
    }

    pub fn dispatch(
        &self,
        request: &InsightDetachedDispatchRequest,
    ) -> InsightDetachedDispatchResult {
        let created_at_ms = chrono::Utc::now().timestamp_millis();
        let sequence = self.next_job_id.fetch_add(1, Ordering::Relaxed);
        let job_id = format!("detached-{}-{sequence:04}", created_at_ms.max(0));
        let job = InsightDetachedJob {
            job_id: job_id.clone(),
            parent_job_id: None,
            mode: request.mode,
            status: InsightDetachedJobStatus::Queued,
            agent: request.agent.clone(),
            team: request.team.clone(),
            chain: request.chain.clone(),
            created_at_ms,
            started_at_ms: None,
            finished_at_ms: None,
            current_step_index: None,
            step_count: estimated_step_count(&self.supervisor, request),
            output_excerpt: None,
            stdout_excerpt: None,
            stderr_excerpt: None,
            error: None,
            fallback_used: false,
            step_results: Vec::new(),
        };

        {
            let mut jobs = self
                .jobs
                .lock()
                .expect("detached insight jobs lock poisoned");
            jobs.insert(job_id.clone(), job.clone());
        }
        persist_job(&self.store_path, &job);

        let jobs = Arc::clone(&self.jobs);
        let supervisor = self.supervisor.clone();
        let request = request.clone();
        let request_mode = request.mode;
        let store_path = self.store_path.clone();
        let running_pids = Arc::clone(&self.running_pids);
        let cancelled_jobs = Arc::clone(&self.cancelled_jobs);
        let background_job_id = job_id.clone();
        std::thread::spawn(move || {
            let started_at_ms = chrono::Utc::now().timestamp_millis();
            {
                let mut jobs = jobs.lock().expect("detached insight jobs lock poisoned");
                let Some(entry) = jobs.get_mut(&background_job_id) else {
                    return;
                };
                if entry.status == InsightDetachedJobStatus::Cancelled {
                    return;
                }
                entry.status = InsightDetachedJobStatus::Running;
                entry.started_at_ms = Some(started_at_ms);
                persist_job(&store_path, entry);
            }

            let result = execute_detached_request(
                &supervisor,
                &background_job_id,
                &request,
                &running_pids,
                &cancelled_jobs,
                &jobs,
                &store_path,
            );
            let finished_at_ms = chrono::Utc::now().timestamp_millis();
            let terminal_status = if result
                .steps
                .iter()
                .any(|step| step.status.eq_ignore_ascii_case("cancelled"))
            {
                InsightDetachedJobStatus::Cancelled
            } else if result
                .steps
                .iter()
                .any(|step| step.status.eq_ignore_ascii_case("error"))
            {
                InsightDetachedJobStatus::Error
            } else if result.fallback_used {
                InsightDetachedJobStatus::Fallback
            } else {
                InsightDetachedJobStatus::Success
            };
            let success_count = result
                .steps
                .iter()
                .filter(|step| step.status.eq_ignore_ascii_case("success"))
                .count();
            let cancelled_count = result
                .steps
                .iter()
                .filter(|step| step.status.eq_ignore_ascii_case("cancelled"))
                .count();
            let fallback_count = result
                .steps
                .len()
                .saturating_sub(success_count + cancelled_count);
            let output_excerpt = Some(truncate_chars(
                format!(
                    "{} | success={} fallback={} cancelled={}",
                    result.summary, success_count, fallback_count, cancelled_count
                ),
                320,
            ))
            .or_else(|| {
                result
                    .steps
                    .iter()
                    .find_map(|step| step.output_excerpt.clone())
            });
            let stdout_excerpt = result
                .steps
                .iter()
                .find_map(|step| step.stdout_excerpt.clone())
                .or_else(|| {
                    result
                        .steps
                        .iter()
                        .find_map(|step| step.output_excerpt.clone())
                });
            let stderr_excerpt = result
                .steps
                .iter()
                .find_map(|step| step.stderr_excerpt.clone());
            let error = result.steps.iter().find_map(|step| step.error.clone());

            let mut jobs = jobs.lock().expect("detached insight jobs lock poisoned");
            if let Some(entry) = jobs.get_mut(&background_job_id) {
                if entry.status == InsightDetachedJobStatus::Cancelled {
                    entry.finished_at_ms.get_or_insert(finished_at_ms);
                    if entry.output_excerpt.is_none() {
                        entry.output_excerpt = output_excerpt;
                    }
                    if entry.stdout_excerpt.is_none() {
                        entry.stdout_excerpt = stdout_excerpt;
                    }
                    if entry.stderr_excerpt.is_none() {
                        entry.stderr_excerpt = stderr_excerpt;
                    }
                    if entry.error.is_none() {
                        entry.error = error;
                    }
                    if entry.step_results.is_empty() {
                        entry.step_results = result.steps.clone();
                    }
                } else {
                    entry.status = terminal_status;
                    entry.finished_at_ms = Some(finished_at_ms);
                    entry.current_step_index = Some(result.steps.len());
                    entry.step_count =
                        Some(result.steps.len().max(entry.step_count.unwrap_or_default()));
                    entry.output_excerpt = output_excerpt;
                    entry.stdout_excerpt = stdout_excerpt;
                    entry.stderr_excerpt = stderr_excerpt;
                    entry.error = error;
                    entry.fallback_used = result.fallback_used;
                    entry.step_results = result.steps.clone();
                }
                persist_job(&store_path, entry);
            }
            clear_cancelled_job(&cancelled_jobs, &background_job_id);
        });

        InsightDetachedDispatchResult {
            job,
            status: "queued".to_string(),
            summary: format!(
                "detached {} queued as {}",
                detached_mode_label(request_mode),
                job_id
            ),
            accepted: true,
            fallback_used: false,
        }
    }

    pub fn status(&self, request: &InsightDetachedStatusRequest) -> InsightDetachedStatusResult {
        let mut jobs = list_persisted_jobs(&self.store_path);
        if jobs.is_empty() {
            let memory_jobs = self
                .jobs
                .lock()
                .expect("detached insight jobs lock poisoned");
            jobs = memory_jobs.values().cloned().collect::<Vec<_>>();
        }
        jobs.sort_by(|a, b| {
            b.created_at_ms
                .cmp(&a.created_at_ms)
                .then_with(|| a.job_id.cmp(&b.job_id))
        });
        if let Some(job_id) = request.job_id.as_deref() {
            jobs.retain(|job| job.job_id == job_id || job.parent_job_id.as_deref() == Some(job_id));
        }
        if let Some(limit) = request.limit {
            jobs.truncate(limit);
        }
        let active_jobs = jobs
            .iter()
            .filter(|job| {
                matches!(
                    job.status,
                    InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running
                )
            })
            .count();
        InsightDetachedStatusResult {
            status: if jobs.is_empty() { "idle" } else { "ok" }.to_string(),
            jobs,
            active_jobs,
            fallback_used: false,
        }
    }

    pub fn cancel(&self, request: &InsightDetachedCancelRequest) -> InsightDetachedCancelResult {
        let mut jobs = self
            .jobs
            .lock()
            .expect("detached insight jobs lock poisoned");
        let Some(existing) = jobs.get(&request.job_id).cloned() else {
            return InsightDetachedCancelResult {
                job_id: request.job_id.clone(),
                status: InsightDetachedJobStatus::Error,
                summary: format!("detached job not found: {}", request.job_id),
                cancelled: false,
                fallback_used: true,
            };
        };
        let child_ids = if existing.parent_job_id.is_none() {
            jobs.values()
                .filter(|child| child.parent_job_id.as_deref() == Some(existing.job_id.as_str()))
                .map(|child| child.job_id.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let Some(job) = jobs.get_mut(&request.job_id) else {
            return InsightDetachedCancelResult {
                job_id: request.job_id.clone(),
                status: InsightDetachedJobStatus::Error,
                summary: format!("detached job not found: {}", request.job_id),
                cancelled: false,
                fallback_used: true,
            };
        };

        match job.status {
            InsightDetachedJobStatus::Queued => {
                record_cancelled_job(&self.cancelled_jobs, &job.job_id);
                for child_id in &child_ids {
                    record_cancelled_job(&self.cancelled_jobs, child_id);
                }
                job.status = InsightDetachedJobStatus::Cancelled;
                job.finished_at_ms = Some(chrono::Utc::now().timestamp_millis());
                job.error = request
                    .reason
                    .as_ref()
                    .map(|reason| format!("cancelled before start: {reason}"))
                    .or_else(|| Some("cancelled before start".to_string()));
                persist_job(&self.store_path, job);
                let response_job_id = job.job_id.clone();
                let response_status = job.status;
                let response_summary =
                    format!("detached job {} cancelled before start", job.job_id);
                let _ = job;
                cancel_child_jobs(
                    &mut jobs,
                    &self.store_path,
                    &child_ids,
                    request.reason.as_deref(),
                );
                InsightDetachedCancelResult {
                    job_id: response_job_id,
                    status: response_status,
                    summary: response_summary,
                    cancelled: true,
                    fallback_used: false,
                }
            }
            InsightDetachedJobStatus::Running => {
                record_cancelled_job(&self.cancelled_jobs, &job.job_id);
                for child_id in &child_ids {
                    record_cancelled_job(&self.cancelled_jobs, child_id);
                }
                let cancelled = terminate_running_job(&self.running_pids, &job.job_id)
                    || child_ids
                        .iter()
                        .any(|child_id| terminate_running_job(&self.running_pids, child_id));
                job.status = InsightDetachedJobStatus::Cancelled;
                job.finished_at_ms = Some(chrono::Utc::now().timestamp_millis());
                job.error = request
                    .reason
                    .as_ref()
                    .map(|reason| format!("cancelled while running: {reason}"))
                    .or_else(|| Some("cancelled while running".to_string()));
                persist_job(&self.store_path, job);
                let response_job_id = job.job_id.clone();
                let response_status = job.status;
                let response_summary = if cancelled {
                    format!("detached job {} terminated", job.job_id)
                } else {
                    format!(
                        "detached job {} marked cancelled; no live subprocesses were found",
                        job.job_id
                    )
                };
                let _ = job;
                cancel_child_jobs(
                    &mut jobs,
                    &self.store_path,
                    &child_ids,
                    request.reason.as_deref(),
                );
                InsightDetachedCancelResult {
                    job_id: response_job_id,
                    status: response_status,
                    summary: response_summary,
                    cancelled,
                    fallback_used: !cancelled,
                }
            }
            status => InsightDetachedCancelResult {
                job_id: job.job_id.clone(),
                status,
                summary: format!(
                    "detached job {} already terminal ({})",
                    job.job_id,
                    detached_status_label(status)
                ),
                cancelled: false,
                fallback_used: false,
            },
        }
    }
}

fn estimated_step_count(
    supervisor: &InsightSupervisor,
    request: &InsightDetachedDispatchRequest,
) -> Option<usize> {
    match request.mode {
        InsightDetachedMode::Dispatch => Some(1),
        InsightDetachedMode::Chain => request
            .chain
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|chain_name| {
                supervisor
                    .load_chain_manifest()
                    .get(chain_name)
                    .map(|chain| chain.steps.len())
            }),
        InsightDetachedMode::Parallel => request
            .team
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|team_name| supervisor.load_team_manifest().get(team_name).map(Vec::len)),
    }
}

fn detached_mode_label(mode: InsightDetachedMode) -> &'static str {
    match mode {
        InsightDetachedMode::Dispatch => "dispatch",
        InsightDetachedMode::Chain => "chain",
        InsightDetachedMode::Parallel => "parallel",
    }
}

fn detached_status_label(status: InsightDetachedJobStatus) -> &'static str {
    match status {
        InsightDetachedJobStatus::Queued => "queued",
        InsightDetachedJobStatus::Running => "running",
        InsightDetachedJobStatus::Success => "success",
        InsightDetachedJobStatus::Fallback => "fallback",
        InsightDetachedJobStatus::Error => "error",
        InsightDetachedJobStatus::Cancelled => "cancelled",
        InsightDetachedJobStatus::Stale => "stale",
    }
}

fn execute_detached_request(
    supervisor: &InsightSupervisor,
    job_id: &str,
    request: &InsightDetachedDispatchRequest,
    running_pids: &Arc<Mutex<BTreeMap<String, BTreeSet<u32>>>>,
    cancelled_jobs: &Arc<Mutex<BTreeSet<String>>>,
    jobs: &Arc<Mutex<BTreeMap<String, InsightDetachedJob>>>,
    store_path: &Path,
) -> InsightDispatchResult {
    let started = Instant::now();
    let (mode, mut steps) = match request.mode {
        InsightDetachedMode::Dispatch => {
            let agent = request
                .agent
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(DEFAULT_DISPATCH_AGENT)
                .to_string();
            let step = run_agent_cancellable(
                supervisor,
                job_id,
                &agent,
                &request.input,
                running_pids,
                cancelled_jobs,
            );
            update_detached_job_progress(
                jobs,
                store_path,
                job_id,
                1,
                Some(1),
                Some(format!("dispatch completed: {agent}")),
                step.error.clone(),
            );
            (InsightDispatchMode::Dispatch, vec![step])
        }
        InsightDetachedMode::Chain => {
            let chain_name = request
                .chain
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(DEFAULT_CHAIN);
            (
                InsightDispatchMode::Chain,
                run_chain_cancellable(
                    supervisor,
                    job_id,
                    chain_name,
                    &request.input,
                    running_pids,
                    cancelled_jobs,
                    jobs,
                    store_path,
                ),
            )
        }
        InsightDetachedMode::Parallel => {
            let team_name = request
                .team
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(DEFAULT_TEAM);
            (
                InsightDispatchMode::Parallel,
                run_parallel_cancellable(
                    supervisor,
                    job_id,
                    team_name,
                    &request.input,
                    running_pids,
                    cancelled_jobs,
                    jobs,
                    store_path,
                ),
            )
        }
    };

    if steps.is_empty() {
        steps.push(InsightDispatchStepResult {
            agent: "insight-supervisor".to_string(),
            status: "fallback".to_string(),
            output_excerpt: None,
            stdout_excerpt: None,
            stderr_excerpt: None,
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

fn run_chain_cancellable(
    supervisor: &InsightSupervisor,
    job_id: &str,
    chain_name: &str,
    original_input: &str,
    running_pids: &Arc<Mutex<BTreeMap<String, BTreeSet<u32>>>>,
    cancelled_jobs: &Arc<Mutex<BTreeSet<String>>>,
    jobs: &Arc<Mutex<BTreeMap<String, InsightDetachedJob>>>,
    store_path: &Path,
) -> Vec<InsightDispatchStepResult> {
    let chains = supervisor.load_chain_manifest();
    let Some(chain) = chains.get(chain_name) else {
        return vec![InsightDispatchStepResult {
            agent: "insight-supervisor".to_string(),
            status: "fallback".to_string(),
            output_excerpt: None,
            stdout_excerpt: None,
            stderr_excerpt: None,
            error: Some(format!("chain not found: {chain_name}")),
        }];
    };

    let mut prior_output = original_input.to_string();
    let mut results = Vec::new();
    let total_steps = chain.steps.len();
    for step in &chain.steps {
        if job_cancelled(job_id, cancelled_jobs) {
            results.push(InsightDispatchStepResult {
                agent: step.agent.clone(),
                status: "cancelled".to_string(),
                output_excerpt: None,
                stdout_excerpt: None,
                stderr_excerpt: None,
                error: Some("cancelled before step start".to_string()),
            });
            break;
        }
        let prompt = step
            .prompt
            .as_deref()
            .unwrap_or("$INPUT")
            .replace("$ORIGINAL", original_input)
            .replace("$INPUT", &prior_output);
        let result = run_agent_cancellable(
            supervisor,
            job_id,
            &step.agent,
            &prompt,
            running_pids,
            cancelled_jobs,
        );
        if let Some(output) = result.output_excerpt.as_ref() {
            prior_output = output.clone();
        }
        let stop = result.status == "cancelled";
        let completed = results.len() + 1;
        update_detached_job_progress(
            jobs,
            store_path,
            job_id,
            completed,
            Some(total_steps),
            Some(format!(
                "chain step {completed}/{total_steps}: {}",
                step.agent
            )),
            result.error.clone(),
        );
        results.push(result);
        if stop {
            break;
        }
    }

    results
}

fn run_parallel_cancellable(
    supervisor: &InsightSupervisor,
    job_id: &str,
    team_name: &str,
    input: &str,
    running_pids: &Arc<Mutex<BTreeMap<String, BTreeSet<u32>>>>,
    cancelled_jobs: &Arc<Mutex<BTreeSet<String>>>,
    jobs: &Arc<Mutex<BTreeMap<String, InsightDetachedJob>>>,
    store_path: &Path,
) -> Vec<InsightDispatchStepResult> {
    let teams = supervisor.load_team_manifest();
    let Some(agents) = teams.get(team_name) else {
        return vec![InsightDispatchStepResult {
            agent: "insight-supervisor".to_string(),
            status: "fallback".to_string(),
            output_excerpt: None,
            stdout_excerpt: None,
            stderr_excerpt: None,
            error: Some(format!("team not found: {team_name}")),
        }];
    };

    let limit = resolve_parallel_limit().min(agents.len().max(1));
    let (tx, rx) = std::sync::mpsc::channel();
    let mut in_flight = 0usize;
    let mut next_index = 0usize;
    let mut results = vec![None; agents.len()];
    let mut completed = 0usize;

    while next_index < agents.len() || in_flight > 0 {
        while in_flight < limit && next_index < agents.len() {
            let supervisor = supervisor.clone();
            let agent_name = agents[next_index].clone();
            let input_text = input.to_string();
            let parent_job_id = job_id.to_string();
            let child_job_id = format!("{parent_job_id}::child::{:02}", next_index + 1);
            let running_pids = Arc::clone(running_pids);
            let cancelled_jobs = Arc::clone(cancelled_jobs);
            let tx = tx.clone();
            let index = next_index;
            create_parallel_child_job(
                jobs,
                store_path,
                &parent_job_id,
                &child_job_id,
                &agent_name,
                index,
                agents.len(),
            );
            std::thread::spawn(move || {
                let result = run_agent_cancellable(
                    &supervisor,
                    &child_job_id,
                    &agent_name,
                    &input_text,
                    &running_pids,
                    &cancelled_jobs,
                );
                let _ = tx.send((index, child_job_id, result));
            });
            next_index += 1;
            in_flight += 1;
        }

        let Ok((index, child_job_id, result)) = rx.recv() else {
            break;
        };
        in_flight = in_flight.saturating_sub(1);
        completed += 1;
        finalize_parallel_child_job(jobs, store_path, &child_job_id, &result);
        update_detached_job_progress(
            jobs,
            store_path,
            job_id,
            completed,
            Some(agents.len()),
            Some(format!(
                "parallel child {completed}/{}: {}",
                agents.len(),
                result.agent
            )),
            result.error.clone(),
        );
        results[index] = Some(result);
        if job_cancelled(job_id, cancelled_jobs) {
            break;
        }
    }

    results.into_iter().flatten().collect::<Vec<_>>()
}

fn run_agent_cancellable(
    supervisor: &InsightSupervisor,
    job_id: &str,
    agent: &str,
    input: &str,
    running_pids: &Arc<Mutex<BTreeMap<String, BTreeSet<u32>>>>,
    cancelled_jobs: &Arc<Mutex<BTreeSet<String>>>,
) -> InsightDispatchStepResult {
    let agent_file = supervisor
        .project_root
        .join(AGENTS_DIR)
        .join(format!("{agent}.md"));
    if !agent_file.exists() {
        return InsightDispatchStepResult {
            agent: agent.to_string(),
            status: "fallback".to_string(),
            output_excerpt: None,
            stdout_excerpt: None,
            stderr_excerpt: None,
            error: Some(format!(
                "agent definition missing: {}",
                relativize(&supervisor.project_root, &agent_file)
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
            stdout_excerpt: None,
            stderr_excerpt: None,
            error: Some("insight subprocess command not configured".to_string()),
        };
    };

    let mut command = Command::new("bash");
    super::configure_mind_child_std_command_env(
        &mut command,
        vec![
            ("AOC_INSIGHT_AGENT".to_string(), agent.to_string()),
            ("AOC_INSIGHT_INPUT".to_string(), input.to_string()),
            (
                "AOC_INSIGHT_AGENT_FILE".to_string(),
                relativize(&supervisor.project_root, &agent_file),
            ),
        ],
    );
    command
        .arg("-lc")
        .arg(cmdline)
        .current_dir(&supervisor.project_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    match command.spawn() {
        Ok(child) => {
            let pid = child.id();
            register_running_pid(running_pids, job_id, pid);
            let output = child.wait_with_output();
            unregister_running_pid(running_pids, job_id, pid);
            match output {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    if job_cancelled(job_id, cancelled_jobs) || terminated_by_signal(&output.status)
                    {
                        InsightDispatchStepResult {
                            agent: agent.to_string(),
                            status: "cancelled".to_string(),
                            output_excerpt: Some(truncate_chars(stdout.clone(), 280))
                                .filter(|v| !v.is_empty()),
                            stdout_excerpt: Some(truncate_chars(stdout, 280))
                                .filter(|v| !v.is_empty()),
                            stderr_excerpt: Some(truncate_chars(stderr.clone(), 240))
                                .filter(|v| !v.is_empty()),
                            error: Some(truncate_chars(
                                if stderr.is_empty() {
                                    "detached job cancelled while running".to_string()
                                } else {
                                    stderr
                                },
                                320,
                            )),
                        }
                    } else if output.status.success() {
                        InsightDispatchStepResult {
                            agent: agent.to_string(),
                            status: "success".to_string(),
                            output_excerpt: Some(truncate_chars(stdout.clone(), 400)),
                            stdout_excerpt: Some(truncate_chars(stdout, 400))
                                .filter(|v| !v.is_empty()),
                            stderr_excerpt: Some(truncate_chars(stderr, 240))
                                .filter(|v| !v.is_empty()),
                            error: None,
                        }
                    } else {
                        InsightDispatchStepResult {
                            agent: agent.to_string(),
                            status: "fallback".to_string(),
                            output_excerpt: Some(truncate_chars(stdout.clone(), 280)),
                            stdout_excerpt: Some(truncate_chars(stdout, 280))
                                .filter(|v| !v.is_empty()),
                            stderr_excerpt: Some(truncate_chars(stderr.clone(), 240))
                                .filter(|v| !v.is_empty()),
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
                    status: if job_cancelled(job_id, cancelled_jobs) {
                        "cancelled".to_string()
                    } else {
                        "fallback".to_string()
                    },
                    output_excerpt: None,
                    stdout_excerpt: None,
                    stderr_excerpt: None,
                    error: Some(format!("failed while waiting for agent subprocess: {err}")),
                },
            }
        }
        Err(err) => InsightDispatchStepResult {
            agent: agent.to_string(),
            status: "fallback".to_string(),
            output_excerpt: None,
            stdout_excerpt: None,
            stderr_excerpt: None,
            error: Some(format!("failed to spawn agent subprocess: {err}")),
        },
    }
}

fn register_running_pid(
    running_pids: &Arc<Mutex<BTreeMap<String, BTreeSet<u32>>>>,
    job_id: &str,
    pid: u32,
) {
    let mut map = running_pids.lock().expect("running pid lock poisoned");
    map.entry(job_id.to_string()).or_default().insert(pid);
}

fn unregister_running_pid(
    running_pids: &Arc<Mutex<BTreeMap<String, BTreeSet<u32>>>>,
    job_id: &str,
    pid: u32,
) {
    let mut map = running_pids.lock().expect("running pid lock poisoned");
    if let Some(pids) = map.get_mut(job_id) {
        pids.remove(&pid);
        if pids.is_empty() {
            map.remove(job_id);
        }
    }
}

fn record_cancelled_job(cancelled_jobs: &Arc<Mutex<BTreeSet<String>>>, job_id: &str) {
    let mut set = cancelled_jobs.lock().expect("cancelled jobs lock poisoned");
    set.insert(job_id.to_string());
}

fn clear_cancelled_job(cancelled_jobs: &Arc<Mutex<BTreeSet<String>>>, job_id: &str) {
    let mut set = cancelled_jobs.lock().expect("cancelled jobs lock poisoned");
    set.remove(job_id);
}

fn job_cancelled(job_id: &str, cancelled_jobs: &Arc<Mutex<BTreeSet<String>>>) -> bool {
    let set = cancelled_jobs.lock().expect("cancelled jobs lock poisoned");
    set.contains(job_id)
}

fn terminate_running_job(
    running_pids: &Arc<Mutex<BTreeMap<String, BTreeSet<u32>>>>,
    job_id: &str,
) -> bool {
    let pids = {
        let map = running_pids.lock().expect("running pid lock poisoned");
        map.get(job_id).cloned().unwrap_or_default()
    };
    if pids.is_empty() {
        return false;
    }

    let mut terminated_any = false;
    for pid in &pids {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        terminated_any = true;
    }
    std::thread::sleep(Duration::from_millis(200));
    for pid in &pids {
        if process_is_alive(*pid) {
            let _ = Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    }
    terminated_any
}

fn process_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn terminated_by_signal(status: &std::process::ExitStatus) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.signal().is_some()
    }
    #[cfg(not(unix))]
    {
        !status.success()
    }
}

fn create_parallel_child_job(
    jobs: &Arc<Mutex<BTreeMap<String, InsightDetachedJob>>>,
    store_path: &Path,
    parent_job_id: &str,
    child_job_id: &str,
    agent: &str,
    index: usize,
    total: usize,
) {
    let mut jobs = jobs.lock().expect("detached insight jobs lock poisoned");
    let child = InsightDetachedJob {
        job_id: child_job_id.to_string(),
        parent_job_id: Some(parent_job_id.to_string()),
        mode: InsightDetachedMode::Dispatch,
        status: InsightDetachedJobStatus::Queued,
        agent: Some(agent.to_string()),
        team: None,
        chain: None,
        created_at_ms: chrono::Utc::now().timestamp_millis(),
        started_at_ms: None,
        finished_at_ms: None,
        current_step_index: Some(0),
        step_count: Some(1),
        output_excerpt: Some(format!(
            "parallel child {}/{} queued: {agent}",
            index + 1,
            total
        )),
        stdout_excerpt: None,
        stderr_excerpt: None,
        error: None,
        fallback_used: false,
        step_results: Vec::new(),
    };
    jobs.insert(child_job_id.to_string(), child.clone());
    persist_job(store_path, &child);
}

fn finalize_parallel_child_job(
    jobs: &Arc<Mutex<BTreeMap<String, InsightDetachedJob>>>,
    store_path: &Path,
    child_job_id: &str,
    result: &InsightDispatchStepResult,
) {
    let mut jobs = jobs.lock().expect("detached insight jobs lock poisoned");
    let Some(job) = jobs.get_mut(child_job_id) else {
        return;
    };
    job.status = match result.status.as_str() {
        "success" => InsightDetachedJobStatus::Success,
        "cancelled" => InsightDetachedJobStatus::Cancelled,
        "error" => InsightDetachedJobStatus::Error,
        _ => InsightDetachedJobStatus::Fallback,
    };
    job.started_at_ms.get_or_insert(job.created_at_ms);
    job.finished_at_ms = Some(chrono::Utc::now().timestamp_millis());
    job.current_step_index = Some(1);
    job.step_count = Some(1);
    job.output_excerpt = result.output_excerpt.clone();
    job.stdout_excerpt = result.stdout_excerpt.clone();
    job.stderr_excerpt = result.stderr_excerpt.clone();
    job.error = result.error.clone();
    job.fallback_used = job.status == InsightDetachedJobStatus::Fallback;
    job.step_results = vec![result.clone()];
    persist_job(store_path, job);
}

fn cancel_child_jobs(
    jobs: &mut BTreeMap<String, InsightDetachedJob>,
    store_path: &Path,
    child_ids: &[String],
    reason: Option<&str>,
) {
    for child_id in child_ids {
        if let Some(child) = jobs.get_mut(child_id) {
            if matches!(
                child.status,
                InsightDetachedJobStatus::Success
                    | InsightDetachedJobStatus::Fallback
                    | InsightDetachedJobStatus::Error
                    | InsightDetachedJobStatus::Cancelled
                    | InsightDetachedJobStatus::Stale
            ) {
                continue;
            }
            child.status = InsightDetachedJobStatus::Cancelled;
            child.finished_at_ms = Some(chrono::Utc::now().timestamp_millis());
            child.stdout_excerpt = None;
            child.stderr_excerpt = None;
            child.error = Some(match reason {
                Some(reason) if !reason.trim().is_empty() => {
                    format!("cancelled by parent: {reason}")
                }
                _ => "cancelled by parent".to_string(),
            });
            child.step_results = vec![InsightDispatchStepResult {
                agent: child
                    .agent
                    .clone()
                    .unwrap_or_else(|| "insight-supervisor".to_string()),
                status: "cancelled".to_string(),
                output_excerpt: child.output_excerpt.clone(),
                stdout_excerpt: None,
                stderr_excerpt: None,
                error: child.error.clone(),
            }];
            persist_job(store_path, child);
        }
    }
}

fn update_detached_job_progress(
    jobs: &Arc<Mutex<BTreeMap<String, InsightDetachedJob>>>,
    store_path: &Path,
    job_id: &str,
    completed_steps: usize,
    total_steps: Option<usize>,
    output_excerpt: Option<String>,
    error: Option<String>,
) {
    let mut jobs = jobs.lock().expect("detached insight jobs lock poisoned");
    let Some(job) = jobs.get_mut(job_id) else {
        return;
    };
    job.current_step_index = Some(completed_steps);
    if total_steps.is_some() {
        job.step_count = total_steps;
    }
    if let Some(output_excerpt) = output_excerpt.filter(|value| !value.trim().is_empty()) {
        job.output_excerpt = Some(truncate_chars(output_excerpt, 320));
    }
    if let Some(error) = error.filter(|value| !value.trim().is_empty()) {
        job.error = Some(truncate_chars(error, 320));
    }
    persist_job(store_path, job);
}

fn resolve_parallel_limit() -> usize {
    std::env::var("AOC_INSIGHT_DETACHED_PARALLEL_LIMIT")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_PARALLEL_CONCURRENCY)
}

fn persist_job(store_path: &Path, job: &InsightDetachedJob) {
    if let Ok(store) = MindStore::open(store_path) {
        let worker_kind = match job.mode {
            InsightDetachedMode::Dispatch => Some("specialist"),
            InsightDetachedMode::Chain => Some("chain_step"),
            InsightDetachedMode::Parallel => Some("team_fanout"),
        };
        let _ = store.upsert_detached_insight_job("delegated", worker_kind, job);
    }
}

fn recover_persisted_jobs(store_path: &Path) -> Vec<InsightDetachedJob> {
    let Ok(store) = MindStore::open(store_path) else {
        return Vec::new();
    };
    let _ = store.mark_detached_insight_jobs_stale(
        "delegated",
        "wrapper restarted before detached result was observed",
    );
    store
        .detached_insight_jobs(Some("delegated"), Some(64))
        .unwrap_or_default()
}

fn list_persisted_jobs(store_path: &Path) -> Vec<InsightDetachedJob> {
    let Ok(store) = MindStore::open(store_path) else {
        return Vec::new();
    };
    store
        .detached_insight_jobs(Some("delegated"), Some(64))
        .unwrap_or_default()
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

    #[test]
    fn detached_runtime_can_cancel_running_job() {
        let _guard = env_lock().lock().expect("env lock");
        let root = fixture_root();
        let store_path = root.join("mind.sqlite");
        let old = std::env::var("AOC_INSIGHT_AGENT_CMD").ok();
        let old_parallel_limit = std::env::var("AOC_INSIGHT_DETACHED_PARALLEL_LIMIT").ok();
        std::env::set_var("AOC_INSIGHT_AGENT_CMD", "trap 'exit 143' TERM; sleep 10");

        let runtime = DetachedInsightRuntime::new(&root, &store_path);
        let dispatch = runtime.dispatch(&InsightDetachedDispatchRequest {
            mode: InsightDetachedMode::Dispatch,
            agent: Some("insight-t1-observer".to_string()),
            input: "hello".to_string(),
            ..InsightDetachedDispatchRequest::default()
        });
        let job_id = dispatch.job.job_id.clone();

        let mut saw_running = false;
        for _ in 0..40 {
            let status = runtime.status(&InsightDetachedStatusRequest {
                job_id: Some(job_id.clone()),
                limit: Some(1),
            });
            if status
                .jobs
                .iter()
                .any(|job| job.status == InsightDetachedJobStatus::Running)
            {
                saw_running = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(saw_running, "job never entered running state");

        let cancelled = runtime.cancel(&InsightDetachedCancelRequest {
            job_id: job_id.clone(),
            reason: Some("test cancel".to_string()),
        });
        assert_eq!(cancelled.status, InsightDetachedJobStatus::Cancelled);
        assert!(cancelled.cancelled || cancelled.fallback_used);

        let mut terminal = None;
        for _ in 0..40 {
            let status = runtime.status(&InsightDetachedStatusRequest {
                job_id: Some(job_id.clone()),
                limit: Some(1),
            });
            terminal = status.jobs.into_iter().next();
            if terminal
                .as_ref()
                .map(|job| job.status == InsightDetachedJobStatus::Cancelled)
                .unwrap_or(false)
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(
            terminal.as_ref().map(|job| job.status),
            Some(InsightDetachedJobStatus::Cancelled)
        );

        if let Some(previous) = old {
            std::env::set_var("AOC_INSIGHT_AGENT_CMD", previous);
        } else {
            std::env::remove_var("AOC_INSIGHT_AGENT_CMD");
        }
        if let Some(previous) = old_parallel_limit {
            std::env::set_var("AOC_INSIGHT_DETACHED_PARALLEL_LIMIT", previous);
        } else {
            std::env::remove_var("AOC_INSIGHT_DETACHED_PARALLEL_LIMIT");
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn detached_parallel_runtime_honors_bounded_concurrency() {
        let _guard = env_lock().lock().expect("env lock");
        let root = fixture_root();
        let store_path = root.join("mind.sqlite");
        let script_path = root.join("parallel-check.sh");
        let lock_dir = root.join("parallel-lock");
        let seen_file = root.join("parallel-seen.log");
        let overlap_file = root.join("parallel-overlap.log");
        let mut script = fs::File::create(&script_path).expect("script");
        writeln!(
            script,
            "#!/usr/bin/env bash\nif ! mkdir '{}' 2>/dev/null; then echo overlap >> '{}'; fi\necho \"$AOC_INSIGHT_AGENT\" >> '{}'\nsleep 0.2\nrmdir '{}' 2>/dev/null || true\necho done-$AOC_INSIGHT_AGENT",
            lock_dir.to_string_lossy(),
            overlap_file.to_string_lossy(),
            seen_file.to_string_lossy(),
            lock_dir.to_string_lossy(),
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
        let old_parallel_limit = std::env::var("AOC_INSIGHT_DETACHED_PARALLEL_LIMIT").ok();
        std::env::set_var(
            "AOC_INSIGHT_AGENT_CMD",
            format!("bash '{}'", script_path.to_string_lossy()),
        );
        std::env::set_var("AOC_INSIGHT_DETACHED_PARALLEL_LIMIT", "1");

        let runtime = DetachedInsightRuntime::new(&root, &store_path);
        let dispatch = runtime.dispatch(&InsightDetachedDispatchRequest {
            mode: InsightDetachedMode::Parallel,
            team: Some("insight-core".to_string()),
            input: "hello".to_string(),
            ..InsightDetachedDispatchRequest::default()
        });
        let job_id = dispatch.job.job_id.clone();

        let mut parent = None;
        for _ in 0..200 {
            let status = runtime.status(&InsightDetachedStatusRequest {
                job_id: Some(job_id.clone()),
                limit: Some(10),
            });
            parent = status.jobs.into_iter().find(|job| job.job_id == job_id);
            if parent
                .as_ref()
                .map(|job| {
                    matches!(
                        job.status,
                        InsightDetachedJobStatus::Success
                            | InsightDetachedJobStatus::Fallback
                            | InsightDetachedJobStatus::Cancelled
                            | InsightDetachedJobStatus::Error
                    )
                })
                .unwrap_or(false)
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let parent = parent.expect("parent job");
        assert_eq!(parent.status, InsightDetachedJobStatus::Success);
        assert_eq!(parent.step_count, Some(2));
        assert_eq!(parent.current_step_index, Some(2));
        let status = runtime.status(&InsightDetachedStatusRequest {
            job_id: Some(job_id.clone()),
            limit: Some(10),
        });
        assert_eq!(status.jobs.len(), 3);
        assert_eq!(
            status
                .jobs
                .iter()
                .filter(|job| job.parent_job_id.as_deref() == Some(job_id.as_str()))
                .count(),
            2
        );
        assert_eq!(parent.step_results.len(), 2);
        assert!(parent.stdout_excerpt.as_deref().is_some());
        assert!(status
            .jobs
            .iter()
            .filter(|job| job.parent_job_id.as_deref() == Some(job_id.as_str()))
            .all(|job| job.step_results.len() == 1));
        let seen = fs::read_to_string(&seen_file).expect("seen file");
        assert!(seen.contains("insight-t1-observer"));
        assert!(seen.contains("insight-t2-reflector"));
        let overlap = fs::read_to_string(&overlap_file).unwrap_or_default();
        assert!(
            overlap.trim().is_empty(),
            "parallel overlap detected: {overlap}"
        );

        if let Some(previous) = old {
            std::env::set_var("AOC_INSIGHT_AGENT_CMD", previous);
        } else {
            std::env::remove_var("AOC_INSIGHT_AGENT_CMD");
        }
        if let Some(previous) = old_parallel_limit {
            std::env::set_var("AOC_INSIGHT_DETACHED_PARALLEL_LIMIT", previous);
        } else {
            std::env::remove_var("AOC_INSIGHT_DETACHED_PARALLEL_LIMIT");
        }
        let _ = fs::remove_dir_all(root);
    }
}

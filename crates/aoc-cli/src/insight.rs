use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::Value;
use std::{env, path::PathBuf, time::Duration};

use aoc_core::{
    insight_contracts::{
        InsightRetrievalMode, InsightRetrievalRequest, InsightRetrievalResult,
        InsightRetrievalScope, InsightStatusResult,
    },
    provenance_contracts::{MindProvenanceExport, MindProvenanceQueryRequest},
};

use crate::overseer::{
    request_command_result, resolve_pulse_socket_path, resolve_session_id, CommandResultView,
};

#[derive(Subcommand, Debug)]
pub enum InsightCommand {
    /// Query Mind-backed retrieval across project/session sources
    Retrieve(RetrieveArgs),
    /// Query Mind provenance / traversal graph
    Provenance(ProvenanceArgs),
    /// Read current insight runtime health/status
    Status(StatusArgs),
}

#[derive(Args, Debug)]
pub struct RetrieveArgs {
    /// Session id to inspect. Falls back to AOC_SESSION_ID.
    #[arg(long)]
    pub session_id: Option<String>,
    /// Target agent id. Defaults to <session>::<AOC_PANE_ID|ZELLIJ_PANE_ID> when available.
    #[arg(long)]
    pub target_agent_id: Option<String>,
    /// Socket path override. Falls back to AOC_PULSE_SOCK or the default runtime path.
    #[arg(long)]
    pub socket_path: Option<PathBuf>,
    /// Read timeout in milliseconds.
    #[arg(long, default_value_t = 3000)]
    pub timeout_ms: u64,
    /// Print raw JSON payload.
    #[arg(long, default_value_t = false)]
    pub json: bool,
    #[arg(long, value_enum, default_value_t = ScopeArg::Auto)]
    pub scope: ScopeArg,
    #[arg(long, value_enum, default_value_t = ModeArg::Brief)]
    pub mode: ModeArg,
    #[arg(long)]
    pub active_tag: Option<String>,
    #[arg(long)]
    pub max_results: Option<usize>,
    /// Retrieval query text.
    pub query: String,
}

#[derive(Args, Debug)]
pub struct ProvenanceArgs {
    /// Session id to inspect. Falls back to AOC_SESSION_ID.
    #[arg(long)]
    pub session_id: Option<String>,
    /// Target agent id. Defaults to <session>::<AOC_PANE_ID|ZELLIJ_PANE_ID> when available.
    #[arg(long)]
    pub target_agent_id: Option<String>,
    /// Socket path override. Falls back to AOC_PULSE_SOCK or the default runtime path.
    #[arg(long)]
    pub socket_path: Option<PathBuf>,
    /// Read timeout in milliseconds.
    #[arg(long, default_value_t = 3000)]
    pub timeout_ms: u64,
    /// Print raw JSON payload.
    #[arg(long, default_value_t = false)]
    pub json: bool,
    #[arg(long)]
    pub project_root: Option<String>,
    #[arg(long)]
    pub session_seed: Option<String>,
    #[arg(long)]
    pub conversation_id: Option<String>,
    #[arg(long)]
    pub artifact_id: Option<String>,
    #[arg(long)]
    pub checkpoint_id: Option<String>,
    #[arg(long)]
    pub canon_entry_id: Option<String>,
    #[arg(long)]
    pub task_id: Option<String>,
    #[arg(long)]
    pub file_path: Option<String>,
    #[arg(long)]
    pub active_tag: Option<String>,
    #[arg(long, default_value_t = false)]
    pub include_stale_canon: bool,
    #[arg(long, default_value_t = 64)]
    pub max_nodes: usize,
    #[arg(long, default_value_t = 128)]
    pub max_edges: usize,
}

#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Session id to inspect. Falls back to AOC_SESSION_ID.
    #[arg(long)]
    pub session_id: Option<String>,
    /// Target agent id. Defaults to <session>::<AOC_PANE_ID|ZELLIJ_PANE_ID> when available.
    #[arg(long)]
    pub target_agent_id: Option<String>,
    /// Socket path override. Falls back to AOC_PULSE_SOCK or the default runtime path.
    #[arg(long)]
    pub socket_path: Option<PathBuf>,
    /// Read timeout in milliseconds.
    #[arg(long, default_value_t = 3000)]
    pub timeout_ms: u64,
    /// Print raw JSON payload.
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ScopeArg {
    Session,
    Project,
    Auto,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ModeArg {
    Brief,
    Refs,
    Snips,
}

impl From<ScopeArg> for InsightRetrievalScope {
    fn from(value: ScopeArg) -> Self {
        match value {
            ScopeArg::Session => Self::Session,
            ScopeArg::Project => Self::Project,
            ScopeArg::Auto => Self::Auto,
        }
    }
}

impl From<ModeArg> for InsightRetrievalMode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::Brief => Self::Brief,
            ModeArg::Refs => Self::Refs,
            ModeArg::Snips => Self::Snips,
        }
    }
}

pub fn handle_insight_command(command: InsightCommand) -> Result<()> {
    match command {
        InsightCommand::Retrieve(args) => handle_retrieve(args),
        InsightCommand::Provenance(args) => handle_provenance(args),
        InsightCommand::Status(args) => handle_status(args),
    }
}

fn handle_retrieve(args: RetrieveArgs) -> Result<()> {
    let session_id = resolve_session_id(args.session_id)?;
    let target_agent_id = resolve_target_agent_id(&session_id, args.target_agent_id)?;
    let socket_path = resolve_pulse_socket_path(&session_id, args.socket_path);
    let result = request_command_result(
        &session_id,
        &socket_path,
        Duration::from_millis(args.timeout_ms),
        &format!("insight-cli-retrieve-{}", now_ms()),
        &target_agent_id,
        "insight_retrieve",
        serde_json::to_value(InsightRetrievalRequest {
            query: args.query,
            scope: args.scope.into(),
            mode: args.mode.into(),
            active_tag: args.active_tag,
            max_results: args.max_results,
        })?,
    )?;
    if args.json {
        print_command_result_json(&result)?;
        return Ok(());
    }
    let parsed = parse_result_payload::<InsightRetrievalResult>(&result)?;
    println!(
        "query={} status={} scope={:?}->{:?} hits={} hit_budget={} line_budget_per_hit={}",
        parsed.query,
        parsed.status,
        parsed.scope,
        parsed.resolved_scope,
        parsed.hits.len(),
        parsed.hit_budget,
        parsed.line_budget_per_hit,
    );
    for line in parsed.summary_lines.iter().take(8) {
        println!("- {line}");
    }
    if !parsed.citations.is_empty() {
        println!("citations:");
        for citation in parsed.citations.iter().take(6) {
            println!("- {} -> {}", citation.label, citation.reference);
        }
    }
    Ok(())
}

fn handle_provenance(args: ProvenanceArgs) -> Result<()> {
    let session_id = resolve_session_id(args.session_id)?;
    let target_agent_id = resolve_target_agent_id(&session_id, args.target_agent_id)?;
    let socket_path = resolve_pulse_socket_path(&session_id, args.socket_path);
    let result = request_command_result(
        &session_id,
        &socket_path,
        Duration::from_millis(args.timeout_ms),
        &format!("insight-cli-provenance-{}", now_ms()),
        &target_agent_id,
        "mind_provenance_query",
        serde_json::to_value(MindProvenanceQueryRequest {
            project_root: args.project_root,
            session_id: args.session_seed,
            conversation_id: args.conversation_id,
            artifact_id: args.artifact_id,
            checkpoint_id: args.checkpoint_id,
            canon_entry_id: args.canon_entry_id,
            task_id: args.task_id,
            file_path: args.file_path,
            active_tag: args.active_tag,
            include_stale_canon: args.include_stale_canon,
            max_nodes: args.max_nodes,
            max_edges: args.max_edges,
        })?,
    )?;
    if args.json {
        print_command_result_json(&result)?;
        return Ok(());
    }
    let parsed = parse_result_payload::<MindProvenanceExport>(&result)?;
    println!(
        "status={} nodes={} edges={} truncated={} summary={}",
        parsed.graph.status,
        parsed.graph.nodes.len(),
        parsed.graph.edges.len(),
        parsed.graph.truncated,
        parsed.graph.summary,
    );
    if !parsed.graph.seed_refs.is_empty() {
        println!("seeds: {}", parsed.graph.seed_refs.join(", "));
    }
    if !parsed.mission_control.focus_node_ids.is_empty() {
        println!(
            "focus_nodes: {}",
            parsed.mission_control.focus_node_ids.join(", ")
        );
    }
    Ok(())
}

fn handle_status(args: StatusArgs) -> Result<()> {
    let session_id = resolve_session_id(args.session_id)?;
    let target_agent_id = resolve_target_agent_id(&session_id, args.target_agent_id)?;
    let socket_path = resolve_pulse_socket_path(&session_id, args.socket_path);
    let result = request_command_result(
        &session_id,
        &socket_path,
        Duration::from_millis(args.timeout_ms),
        &format!("insight-cli-status-{}", now_ms()),
        &target_agent_id,
        "insight_status",
        serde_json::json!({}),
    )?;
    if args.json {
        print_command_result_json(&result)?;
        return Ok(());
    }
    let parsed = parse_result_payload::<InsightStatusResult>(&result)?;
    println!(
        "queue_depth={} reflector_enabled={} supervisor_runs={} jobs_completed={} jobs_failed={} last_error={}",
        parsed.queue_depth,
        parsed.reflector_enabled,
        parsed.supervisor_runs,
        parsed.jobs_completed,
        parsed.jobs_failed,
        parsed.last_error.as_deref().unwrap_or("none"),
    );
    Ok(())
}

fn resolve_target_agent_id(session_id: &str, explicit: Option<String>) -> Result<String> {
    if let Some(value) = explicit
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return Ok(value);
    }
    let pane_id = env::var("AOC_PANE_ID")
        .ok()
        .or_else(|| env::var("ZELLIJ_PANE_ID").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(pane_id) = pane_id {
        return Ok(format!("{}::{}", session_id, pane_id));
    }
    bail!("target agent is ambiguous; pass --target-agent-id or run inside an AOC pane with AOC_PANE_ID")
}

fn parse_result_payload<T: serde::de::DeserializeOwned>(result: &CommandResultView) -> Result<T> {
    let payload = result
        .message
        .as_deref()
        .context("command result did not include a payload")?;
    serde_json::from_str(payload).with_context(|| format!("parse {} payload", result.command))
}

fn print_command_result_json(result: &CommandResultView) -> Result<()> {
    let payload = result
        .message
        .as_deref()
        .map(|text| serde_json::from_str::<Value>(text).unwrap_or(Value::String(text.to_string())));
    let view = serde_json::json!({
        "command": result.command,
        "target_agent_id": result.target_agent_id,
        "status": result.status,
        "error_code": result.error_code,
        "payload": payload,
    });
    println!("{}", serde_json::to_string_pretty(&view)?);
    Ok(())
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

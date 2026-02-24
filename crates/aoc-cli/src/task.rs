use anyhow::{anyhow, bail, Context, Result};
use aoc_core::{
    ProjectData, Subtask, TagContext, Task, TaskPrd, TaskPriority, TaskStatus, TAG_PRD_KEY,
};
use chrono::{SecondsFormat, Utc};
use clap::{Args, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Subcommand, Debug)]
#[command(rename_all = "kebab-case")]
pub enum TaskCommand {
    List(TaskListArgs),
    Init(TaskInitArgs),
    #[command(alias = "add-task")]
    Add(TaskAddArgs),
    Show(TaskShowArgs),
    Edit(TaskEditArgs),
    #[command(alias = "rm")]
    Remove(TaskRemoveArgs),
    Done(TaskTargetArgs),
    Reopen(TaskTargetArgs),
    #[command(alias = "set-status")]
    Status(TaskStatusArgs),
    Next(TaskNextArgs),
    Search(TaskSearchArgs),
    Move(TaskMoveArgs),
    Agent(TaskAgentArgs),
    Sync(TaskSyncArgs),
    Tag {
        #[command(subcommand)]
        action: TagCommand,
    },
    #[command(alias = "subtask")]
    Sub {
        #[command(subcommand)]
        action: SubCommand,
    },
    Prd {
        #[command(subcommand)]
        action: PrdCommand,
    },
}

#[derive(Subcommand, Debug)]
#[command(rename_all = "kebab-case")]
pub enum TagCommand {
    List(TagListArgs),
    Add(TagAddArgs),
    Rename(TagRenameArgs),
    Remove(TagRemoveArgs),
    Set(TagSetArgs),
    Current(TagCurrentArgs),
    Prd {
        #[command(subcommand)]
        action: TagPrdCommand,
    },
}

#[derive(Subcommand, Debug)]
#[command(rename_all = "kebab-case")]
pub enum TagPrdCommand {
    Show(TagPrdShowArgs),
    Set(TagPrdSetArgs),
    Clear(TagPrdClearArgs),
    Init(TagPrdInitArgs),
}

#[derive(Subcommand, Debug)]
#[command(rename_all = "kebab-case")]
pub enum SubCommand {
    Add(SubAddArgs),
    Edit(SubEditArgs),
    Remove(SubRemoveArgs),
    Done(SubTargetArgs),
    Reopen(SubTargetArgs),
    Status(SubStatusArgs),
}

#[derive(Subcommand, Debug)]
#[command(rename_all = "kebab-case")]
pub enum PrdCommand {
    Show(PrdShowArgs),
    Set(PrdSetArgs),
    Clear(PrdClearArgs),
    Init(PrdInitArgs),
}

#[derive(Args, Debug)]
pub struct TaskListArgs {
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub status: Option<TaskStatus>,
    #[arg(long, alias = "query")]
    pub search: Option<String>,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub all_tags: bool,
}

#[derive(Args, Debug)]
pub struct TaskInitArgs {
    #[arg(long, default_value = "master")]
    pub tag: String,
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct TaskAddArgs {
    pub title: String,
    #[arg(long, alias = "description")]
    pub desc: Option<String>,
    #[arg(long)]
    pub details: Option<String>,
    #[arg(long, alias = "testStrategy")]
    pub test_strategy: Option<String>,
    #[arg(long)]
    pub priority: Option<TaskPriority>,
    #[arg(long)]
    pub status: Option<TaskStatus>,
    #[arg(long, value_delimiter = ',')]
    pub depends: Vec<String>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub active_agent: bool,
}

#[derive(Args, Debug)]
pub struct TaskShowArgs {
    pub id: String,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct TaskEditArgs {
    pub id: String,
    #[arg(long, alias = "description")]
    pub desc: Option<String>,
    #[arg(long)]
    pub details: Option<String>,
    #[arg(long, alias = "testStrategy")]
    pub test_strategy: Option<String>,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub priority: Option<TaskPriority>,
    #[arg(long)]
    pub status: Option<TaskStatus>,
    #[arg(long, value_delimiter = ',')]
    pub depends: Vec<String>,
    #[arg(long)]
    pub clear_deps: bool,
    #[arg(long, conflicts_with = "inactive_agent")]
    pub active_agent: bool,
    #[arg(long, conflicts_with = "active_agent")]
    pub inactive_agent: bool,
    #[arg(
        long,
        alias = "parentId",
        value_name = "ID",
        conflicts_with = "clear_parent"
    )]
    pub parent_id: Option<String>,
    #[arg(long, conflicts_with = "parent_id")]
    pub clear_parent: bool,
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Args, Debug)]
pub struct TaskRemoveArgs {
    pub id: String,
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Args, Debug)]
pub struct TaskTargetArgs {
    pub id: String,
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Args, Debug)]
pub struct TaskStatusArgs {
    pub id: String,
    pub status: TaskStatus,
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Args, Debug)]
pub struct TaskNextArgs {
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct TaskSearchArgs {
    pub query: String,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct TaskMoveArgs {
    pub id: String,
    #[arg(long)]
    pub to: String,
    #[arg(long)]
    pub from: Option<String>,
}

#[derive(Args, Debug)]
pub struct TaskAgentArgs {
    pub id: String,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long, conflicts_with = "off")]
    pub on: bool,
    #[arg(long, conflicts_with = "on")]
    pub off: bool,
    #[arg(long)]
    pub toggle: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SyncSource {
    Claude,
}

#[derive(Args, Debug)]
pub struct TaskSyncArgs {
    #[arg(long, value_enum)]
    pub from: Option<SyncSource>,
    #[arg(long, value_enum)]
    pub to: Option<SyncSource>,
    #[arg(long)]
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct TagListArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct TagAddArgs {
    pub name: String,
}

#[derive(Args, Debug)]
pub struct TagRenameArgs {
    pub from: String,
    pub to: String,
}

#[derive(Args, Debug)]
pub struct TagRemoveArgs {
    pub name: String,
}

#[derive(Args, Debug)]
pub struct TagSetArgs {
    pub name: String,
}

#[derive(Args, Debug)]
pub struct TagCurrentArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct TagPrdShowArgs {
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct TagPrdSetArgs {
    pub path: String,
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Args, Debug)]
pub struct TagPrdClearArgs {
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Args, Debug)]
pub struct TagPrdInitArgs {
    #[arg(long)]
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct SubAddArgs {
    pub task_id: String,
    pub title: String,
    #[arg(long, alias = "description")]
    pub desc: Option<String>,
    #[arg(long)]
    pub status: Option<TaskStatus>,
    #[arg(long, value_delimiter = ',')]
    pub depends: Vec<String>,
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Args, Debug)]
pub struct SubEditArgs {
    pub task_id: String,
    pub sub_id: u32,
    #[arg(long, alias = "description")]
    pub desc: Option<String>,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub status: Option<TaskStatus>,
    #[arg(long, value_delimiter = ',')]
    pub depends: Vec<String>,
    #[arg(long)]
    pub clear_deps: bool,
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Args, Debug)]
pub struct SubRemoveArgs {
    pub task_id: String,
    pub sub_id: u32,
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Args, Debug)]
pub struct SubTargetArgs {
    pub task_id: String,
    pub sub_id: u32,
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Args, Debug)]
pub struct SubStatusArgs {
    pub task_id: String,
    pub sub_id: u32,
    pub status: TaskStatus,
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Args, Debug)]
pub struct PrdShowArgs {
    pub id: String,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct PrdSetArgs {
    pub id: String,
    pub path: String,
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Args, Debug)]
pub struct PrdClearArgs {
    pub id: String,
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Args, Debug)]
pub struct PrdInitArgs {
    pub id: String,
    #[arg(long)]
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub force: bool,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskmasterConfig {
    global: Option<TaskmasterGlobal>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskmasterGlobal {
    default_tag: Option<String>,
    default_priority: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskmasterState {
    #[serde(default)]
    current_tag: Option<String>,
    #[serde(default)]
    last_updated: Option<String>,
    #[serde(default, flatten)]
    extra: HashMap<String, Value>,
}

struct TaskPaths {
    root: PathBuf,
    tasks_path: PathBuf,
    config_path: PathBuf,
    state_path: PathBuf,
}

struct TaskContext {
    paths: TaskPaths,
    config: Option<TaskmasterConfig>,
    state: TaskmasterState,
}

struct ProjectLoad {
    project: ProjectData,
    exists: bool,
}

pub fn handle_task_command(command: TaskCommand) -> Result<()> {
    let ctx = TaskContext::new()?;

    match command {
        TaskCommand::List(args) => list_tasks(&ctx, &args),
        TaskCommand::Init(args) => init_task_storage(&ctx, &args),
        TaskCommand::Add(args) => add_task(&ctx, &args),
        TaskCommand::Show(args) => show_task(&ctx, &args),
        TaskCommand::Edit(args) => edit_task(&ctx, &args),
        TaskCommand::Remove(args) => remove_task(&ctx, &args),
        TaskCommand::Done(args) => set_task_status(&ctx, &args, TaskStatus::Done),
        TaskCommand::Reopen(args) => set_task_status(&ctx, &args, TaskStatus::Pending),
        TaskCommand::Status(args) => {
            let target = TaskTargetArgs {
                id: args.id.clone(),
                tag: args.tag.clone(),
            };
            set_task_status(&ctx, &target, args.status)
        }
        TaskCommand::Next(args) => next_task(&ctx, &args),
        TaskCommand::Search(args) => search_tasks(&ctx, &args),
        TaskCommand::Move(args) => move_task(&ctx, &args),
        TaskCommand::Agent(args) => toggle_agent(&ctx, &args),
        TaskCommand::Sync(args) => sync_tasks(&ctx, &args),
        TaskCommand::Tag { action } => match action {
            TagCommand::List(args) => list_tags(&ctx, &args),
            TagCommand::Add(args) => add_tag(&ctx, &args),
            TagCommand::Rename(args) => rename_tag(&ctx, &args),
            TagCommand::Remove(args) => remove_tag(&ctx, &args),
            TagCommand::Set(args) => set_tag(&ctx, &args),
            TagCommand::Current(args) => current_tag(&ctx, &args),
            TagCommand::Prd { action } => match action {
                TagPrdCommand::Show(args) => show_tag_prd(&ctx, &args),
                TagPrdCommand::Set(args) => set_tag_prd(&ctx, &args),
                TagPrdCommand::Clear(args) => clear_tag_prd(&ctx, &args),
                TagPrdCommand::Init(args) => init_tag_prd(&ctx, &args),
            },
        },
        TaskCommand::Sub { action } => match action {
            SubCommand::Add(args) => add_subtask(&ctx, &args),
            SubCommand::Edit(args) => edit_subtask(&ctx, &args),
            SubCommand::Remove(args) => remove_subtask(&ctx, &args),
            SubCommand::Done(args) => set_subtask_status(&ctx, &args, TaskStatus::Done),
            SubCommand::Reopen(args) => set_subtask_status(&ctx, &args, TaskStatus::Pending),
            SubCommand::Status(args) => set_subtask_status_explicit(&ctx, &args),
        },
        TaskCommand::Prd { action } => match action {
            PrdCommand::Show(args) => show_task_prd(&ctx, &args),
            PrdCommand::Set(args) => set_task_prd(&ctx, &args),
            PrdCommand::Clear(args) => clear_task_prd(&ctx, &args),
            PrdCommand::Init(args) => init_task_prd(&ctx, &args),
        },
    }
}

impl TaskContext {
    fn new() -> Result<Self> {
        let root = resolve_root()?;
        let tasks_path = root.join(".taskmaster/tasks/tasks.json");
        let config_path = root.join(".taskmaster/config.json");
        let state_path = root.join(".taskmaster/state.json");
        let paths = TaskPaths {
            root,
            tasks_path,
            config_path,
            state_path,
        };
        let config = load_config(&paths);
        let state = load_state(&paths);
        Ok(Self {
            paths,
            config,
            state,
        })
    }

    fn resolve_tag(&self, override_tag: Option<&str>) -> String {
        if let Some(tag) = override_tag {
            return tag.to_string();
        }
        if let Ok(tag) = std::env::var("AOC_TASK_TAG") {
            if !tag.trim().is_empty() {
                return tag;
            }
        }
        if let Ok(tag) = std::env::var("TASKMASTER_TAG") {
            if !tag.trim().is_empty() {
                return tag;
            }
        }
        if let Some(tag) = self.state.current_tag.as_ref() {
            if !tag.trim().is_empty() {
                return tag.clone();
            }
        }
        if let Some(config) = &self.config {
            if let Some(global) = &config.global {
                if let Some(tag) = global.default_tag.as_ref() {
                    if !tag.trim().is_empty() {
                        return tag.clone();
                    }
                }
            }
        }
        "master".to_string()
    }

    fn resolve_priority(&self, override_priority: Option<TaskPriority>) -> TaskPriority {
        if let Some(priority) = override_priority {
            return priority;
        }
        if let Some(config) = &self.config {
            if let Some(global) = &config.global {
                if let Some(priority) = global.default_priority.as_ref() {
                    if let Ok(parsed) = priority.parse::<TaskPriority>() {
                        return parsed;
                    }
                }
            }
        }
        TaskPriority::default()
    }
}

fn resolve_root() -> Result<PathBuf> {
    // The CLI runs inside the project directory - current_dir is truth
    Ok(std::env::current_dir()?)
}

fn load_config(paths: &TaskPaths) -> Option<TaskmasterConfig> {
    let mut candidates = Vec::new();
    if paths.config_path.exists() {
        candidates.push(paths.config_path.clone());
    } else if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".taskmaster/config.json"));
    }

    for path in candidates {
        if let Ok(content) = fs::read_to_string(&path) {
            match serde_json::from_str::<TaskmasterConfig>(&content) {
                Ok(config) => return Some(config),
                Err(err) => {
                    eprintln!("Warning: failed to parse {}: {}", path.display(), err);
                }
            }
        }
    }
    None
}

fn load_state(paths: &TaskPaths) -> TaskmasterState {
    if let Ok(content) = fs::read_to_string(&paths.state_path) {
        if let Ok(state) = serde_json::from_str::<TaskmasterState>(&content) {
            return state;
        }
    }
    TaskmasterState::default()
}

fn update_state(paths: &TaskPaths, mutator: impl FnOnce(&mut TaskmasterState)) -> Result<()> {
    let mut state = load_state(paths);
    mutator(&mut state);
    if let Some(parent) = paths.state_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let payload = serde_json::to_string_pretty(&state)?;
    fs::write(&paths.state_path, payload)
        .with_context(|| format!("Failed to write {}", paths.state_path.display()))?;
    Ok(())
}

fn touch_state(paths: &TaskPaths, current_tag: Option<&str>) -> Result<()> {
    let now = now_timestamp();
    update_state(paths, |state| {
        state.last_updated = Some(now);
        if let Some(tag) = current_tag {
            state.current_tag = Some(tag.to_string());
        }
    })
}

fn load_project(paths: &TaskPaths) -> Result<ProjectLoad> {
    if !paths.tasks_path.exists() {
        return Ok(ProjectLoad {
            project: ProjectData {
                tags: HashMap::new(),
            },
            exists: false,
        });
    }

    let content = fs::read_to_string(&paths.tasks_path)
        .with_context(|| format!("Failed to read {}", paths.tasks_path.display()))?;
    let project = parse_project(&content)
        .with_context(|| format!("Failed to parse {}", paths.tasks_path.display()))?;
    validate_project(&project)?;
    Ok(ProjectLoad {
        project,
        exists: true,
    })
}

fn parse_project(content: &str) -> Result<ProjectData> {
    if let Ok(project) = serde_json::from_str::<ProjectData>(content) {
        return Ok(project);
    }

    let raw: Value = serde_json::from_str(content).context("Tasks file is not valid JSON")?;
    let root = raw
        .as_object()
        .ok_or_else(|| anyhow!("Tasks file root must be a JSON object"))?;

    if let Some(tasks_value) = root.get("tasks") {
        let tasks: Vec<Task> = serde_json::from_value(tasks_value.clone())
            .context("Legacy tasks format has invalid tasks array")?;

        let mut extra = HashMap::new();
        if let Some(metadata) = root.get("metadata") {
            extra.insert("metadata".to_string(), metadata.clone());
        }

        let mut tags = HashMap::new();
        tags.insert("master".to_string(), TagContext { tasks, extra });
        return Ok(ProjectData { tags });
    }

    if let Some(tags_value) = root.get("tags") {
        let tags_obj = tags_value
            .as_object()
            .ok_or_else(|| anyhow!("Legacy wrapped tags format requires object at key 'tags'"))?;

        let mut tags = HashMap::new();
        for (tag_name, tag_ctx_value) in tags_obj {
            let tag_ctx: TagContext = serde_json::from_value(tag_ctx_value.clone())
                .with_context(|| format!("Invalid legacy tag context for '{}'", tag_name))?;
            tags.insert(tag_name.clone(), tag_ctx);
        }
        return Ok(ProjectData { tags });
    }

    bail!(
        "Unsupported tasks format. Expected top-level tags map, legacy {{\"tasks\": [...]}}, or wrapped {{\"tags\": {{...}}}}"
    )
}

fn validate_project(project: &ProjectData) -> Result<()> {
    for (tag, tag_ctx) in &project.tags {
        if let Some(raw_prd) = tag_ctx.extra.get(TAG_PRD_KEY) {
            let prd: TaskPrd = serde_json::from_value(raw_prd.clone())
                .with_context(|| format!("Tag '{}' has malformed {} payload", tag, TAG_PRD_KEY))?;
            if prd.path.trim().is_empty() {
                bail!("Tag '{}' has empty {}.path", tag, TAG_PRD_KEY);
            }
        }
        for task in &tag_ctx.tasks {
            if let Some(prd) = &task.aoc_prd {
                if prd.path.trim().is_empty() {
                    bail!("Task [{}] in tag '{}' has empty aocPrd.path", task.id, tag);
                }
            }
            for sub in &task.subtasks {
                if sub.extra.contains_key("aocPrd") {
                    bail!(
                        "Subtask [{}] in task [{}] tag '{}' has unsupported aocPrd",
                        sub.id,
                        task.id,
                        tag
                    );
                }
            }
        }
    }
    Ok(())
}

fn write_project_file(paths: &TaskPaths, project: &ProjectData) -> Result<()> {
    if let Some(parent) = paths.tasks_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let payload = serde_json::to_string_pretty(project).context("Failed to serialize tasks")?;
    let tmp_path = paths.tasks_path.with_extension("json.tmp");
    fs::write(&tmp_path, payload).context("Failed to write temp tasks file")?;
    fs::rename(&tmp_path, &paths.tasks_path).context("Failed to save tasks file")?;
    Ok(())
}

fn save_project(paths: &TaskPaths, project: &ProjectData) -> Result<()> {
    write_project_file(paths, project)?;
    touch_state(paths, None)?;
    Ok(())
}

fn init_task_storage(ctx: &TaskContext, args: &TaskInitArgs) -> Result<()> {
    let tag = args.tag.trim();
    if tag.is_empty() {
        bail!("Tag cannot be empty");
    }

    let taskmaster_dir = ctx.paths.root.join(".taskmaster");
    fs::create_dir_all(taskmaster_dir.join("tasks")).with_context(|| {
        format!(
            "Failed to create {}",
            taskmaster_dir.join("tasks").display()
        )
    })?;
    fs::create_dir_all(taskmaster_dir.join("docs"))
        .with_context(|| format!("Failed to create {}", taskmaster_dir.join("docs").display()))?;
    fs::create_dir_all(taskmaster_dir.join("docs/prds")).with_context(|| {
        format!(
            "Failed to create {}",
            taskmaster_dir.join("docs/prds").display()
        )
    })?;
    fs::create_dir_all(taskmaster_dir.join("reports")).with_context(|| {
        format!(
            "Failed to create {}",
            taskmaster_dir.join("reports").display()
        )
    })?;
    fs::create_dir_all(taskmaster_dir.join("templates")).with_context(|| {
        format!(
            "Failed to create {}",
            taskmaster_dir.join("templates").display()
        )
    })?;

    if args.force || !ctx.paths.tasks_path.exists() {
        let mut tags = HashMap::new();
        tags.insert(
            tag.to_string(),
            TagContext {
                tasks: Vec::new(),
                extra: HashMap::new(),
            },
        );
        write_project_file(&ctx.paths, &ProjectData { tags })?;
    }

    if args.force || !ctx.paths.state_path.exists() {
        let now = now_timestamp();
        let state_payload = json!({
            "currentTag": tag,
            "lastUpdated": now,
            "lastSwitched": now,
            "branchTagMapping": {},
            "migrationNoticeShown": true
        });
        let payload = serde_json::to_string_pretty(&state_payload)?;
        fs::write(&ctx.paths.state_path, payload)
            .with_context(|| format!("Failed to write {}", ctx.paths.state_path.display()))?;
    } else {
        let tag_owned = tag.to_string();
        update_state(&ctx.paths, |state| {
            if state
                .current_tag
                .as_ref()
                .map(|existing| existing.trim().is_empty())
                .unwrap_or(true)
            {
                state.current_tag = Some(tag_owned);
            }
            if state.last_updated.is_none() {
                state.last_updated = Some(now_timestamp());
            }
        })?;
    }

    println!(
        "Initialized task storage at {}",
        ctx.paths.tasks_path.display()
    );
    Ok(())
}

fn ensure_tag<'a>(project: &'a mut ProjectData, tag: &str) -> &'a mut TagContext {
    project.tags.entry(tag.to_string()).or_insert(TagContext {
        tasks: Vec::new(),
        extra: HashMap::new(),
    })
}

fn find_task_index(ctx: &TagContext, id: &str) -> Option<usize> {
    ctx.tasks.iter().position(|task| task.id == id)
}

fn find_subtask_index(task: &Task, sub_id: u32) -> Option<usize> {
    task.subtasks.iter().position(|sub| sub.id == sub_id)
}

fn task_parent_id(task: &Task) -> Option<&str> {
    task.extra
        .get("parentId")
        .and_then(|value| value.as_str())
        .or_else(|| {
            task.extra
                .get("parentTaskId")
                .and_then(|value| value.as_str())
        })
}

fn next_task_id(project: &ProjectData) -> String {
    let mut max_id = 0u64;
    for ctx in project.tags.values() {
        for task in &ctx.tasks {
            if let Ok(parsed) = task.id.parse::<u64>() {
                if parsed > max_id {
                    max_id = parsed;
                }
            }
        }
    }
    (max_id + 1).to_string()
}

fn next_subtask_id(task: &Task) -> u32 {
    let mut max_id = 0u32;
    for sub in &task.subtasks {
        if sub.id > max_id {
            max_id = sub.id;
        }
    }
    max_id + 1
}

fn now_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn list_tasks(ctx: &TaskContext, args: &TaskListArgs) -> Result<()> {
    let ProjectLoad { project, exists } = load_project(&ctx.paths)?;
    if !exists {
        println!("No tasks.json found at {}", ctx.paths.tasks_path.display());
        return Ok(());
    }

    if args.all_tags {
        if args.json {
            let payload = serde_json::to_string_pretty(&project)?;
            println!("{}", payload);
            return Ok(());
        }

        let mut tags: Vec<_> = project.tags.keys().cloned().collect();
        tags.sort();
        for tag in tags {
            if let Some(tag_ctx) = project.tags.get(&tag) {
                println!("{tag} ({})", tag_ctx.tasks.len());
                print_task_list(tag_ctx, args.status.clone(), args.search.as_deref());
            }
        }
        return Ok(());
    }

    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = match project.tags.get(&tag) {
        Some(ctx) => ctx,
        None => {
            println!("No tag named '{tag}' found.");
            return Ok(());
        }
    };

    if args.json {
        let payload = serde_json::to_string_pretty(tag_ctx)?;
        println!("{}", payload);
        return Ok(());
    }

    print_task_list(tag_ctx, args.status.clone(), args.search.as_deref());
    Ok(())
}

fn print_task_list(tag_ctx: &TagContext, status: Option<TaskStatus>, search: Option<&str>) {
    let query = search.map(|q| q.to_lowercase());
    let status = status.as_ref();
    for task in &tag_ctx.tasks {
        if let Some(status_filter) = status {
            if &task.status != status_filter {
                continue;
            }
        }
        if let Some(query) = &query {
            if !task_matches(task, query) {
                continue;
            }
        }
        println!(
            "- [{}] ({}/{}) {}",
            task.id, task.status, task.priority, task.title
        );
    }
}

fn task_matches(task: &Task, query: &str) -> bool {
    let query = query.to_lowercase();
    if task.title.to_lowercase().contains(&query)
        || task.description.to_lowercase().contains(&query)
        || task.details.to_lowercase().contains(&query)
        || task.test_strategy.to_lowercase().contains(&query)
        || task
            .aoc_prd
            .as_ref()
            .map(|prd| prd.path.to_lowercase().contains(&query))
            .unwrap_or(false)
    {
        return true;
    }
    task.subtasks.iter().any(|sub| {
        sub.title.to_lowercase().contains(&query) || sub.description.to_lowercase().contains(&query)
    })
}

fn add_task(ctx: &TaskContext, args: &TaskAddArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let id = next_task_id(&load.project);
    let tag_ctx = ensure_tag(&mut load.project, &tag);
    let priority = ctx.resolve_priority(args.priority.clone());
    let status = args.status.clone().unwrap_or_default();
    let task = Task {
        id: id.clone(),
        title: args.title.clone(),
        description: args.desc.clone().unwrap_or_default(),
        details: args.details.clone().unwrap_or_default(),
        test_strategy: args.test_strategy.clone().unwrap_or_default(),
        status,
        dependencies: args.depends.clone(),
        priority,
        subtasks: Vec::new(),
        aoc_prd: None,
        updated_at: Some(now_timestamp()),
        active_agent: args.active_agent,
        extra: HashMap::new(),
    };
    tag_ctx.tasks.push(task);
    save_project(&ctx.paths, &load.project)?;
    println!("Added task [{id}] to tag '{tag}'.");
    Ok(())
}

fn show_task(ctx: &TaskContext, args: &TaskShowArgs) -> Result<()> {
    let load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let task = tag_ctx
        .tasks
        .iter()
        .find(|task| task.id == args.id)
        .ok_or_else(|| anyhow::anyhow!("Task [{}] not found in tag '{tag}'.", args.id))?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(task)?);
        return Ok(());
    }

    println!("ID: {}", task.id);
    println!("Title: {}", task.title);
    println!("Status: {}", task.status);
    println!("Priority: {}", task.priority);
    if !task.description.is_empty() {
        println!("Description: {}", task.description);
    }
    if !task.details.is_empty() {
        println!("Details: {}", task.details);
    }
    if !task.test_strategy.is_empty() {
        println!("Test Strategy: {}", task.test_strategy);
    }
    if let Some(prd) = &task.aoc_prd {
        println!("PRD: {}", prd.path);
        if let Some(updated_at) = &prd.updated_at {
            println!("PRD Updated: {}", updated_at);
        }
    }
    if !task.dependencies.is_empty() {
        println!("Depends: {}", task.dependencies.join(", "));
    }
    if let Some(parent_id) = task_parent_id(task) {
        println!("Parent ID: {}", parent_id);
    }
    if !task.subtasks.is_empty() {
        println!("Subtasks:");
        for sub in &task.subtasks {
            println!("  - [{}] {} ({})", sub.id, sub.title, sub.status);
        }
    }
    Ok(())
}

fn edit_task(ctx: &TaskContext, args: &TaskEditArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let task_idx = find_task_index(tag_ctx, &args.id)
        .ok_or_else(|| anyhow::anyhow!("Task [{}] not found in tag '{tag}'.", args.id))?;
    let parent_id_to_set = if let Some(parent_id_raw) = &args.parent_id {
        let parent_id = parent_id_raw.trim();
        if parent_id.is_empty() {
            bail!("Parent ID cannot be empty.");
        }
        if parent_id == args.id {
            bail!("Task [{}] cannot be its own parent.", args.id);
        }
        if find_task_index(tag_ctx, parent_id).is_none() {
            bail!("Parent task [{}] not found in tag '{tag}'.", parent_id);
        }
        Some(parent_id.to_string())
    } else {
        None
    };

    let task = &mut tag_ctx.tasks[task_idx];

    if let Some(title) = &args.title {
        task.title = title.clone();
    }
    if let Some(desc) = &args.desc {
        task.description = desc.clone();
    }
    if let Some(details) = &args.details {
        task.details = details.clone();
    }
    if let Some(strategy) = &args.test_strategy {
        task.test_strategy = strategy.clone();
    }
    if let Some(priority) = args.priority.clone() {
        task.priority = priority;
    }
    if let Some(status) = args.status.clone() {
        task.status = status;
    }
    if args.clear_deps {
        task.dependencies.clear();
    } else if !args.depends.is_empty() {
        task.dependencies = args.depends.clone();
    }
    if args.active_agent {
        task.active_agent = true;
    } else if args.inactive_agent {
        task.active_agent = false;
    }
    if let Some(parent_id) = parent_id_to_set {
        task.extra
            .insert("parentId".to_string(), Value::String(parent_id));
        task.extra.remove("parentTaskId");
    } else if args.clear_parent {
        task.extra.remove("parentId");
        task.extra.remove("parentTaskId");
    }
    task.updated_at = Some(now_timestamp());
    save_project(&ctx.paths, &load.project)?;
    println!("Updated task [{}] in tag '{tag}'.", args.id);
    Ok(())
}

fn remove_task(ctx: &TaskContext, args: &TaskRemoveArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let task_idx = find_task_index(tag_ctx, &args.id)
        .ok_or_else(|| anyhow::anyhow!("Task [{}] not found in tag '{tag}'.", args.id))?;
    tag_ctx.tasks.remove(task_idx);
    save_project(&ctx.paths, &load.project)?;
    println!("Removed task [{}] from tag '{tag}'.", args.id);
    Ok(())
}

fn set_task_status(ctx: &TaskContext, args: &TaskTargetArgs, status: TaskStatus) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let task_idx = find_task_index(tag_ctx, &args.id)
        .ok_or_else(|| anyhow::anyhow!("Task [{}] not found in tag '{tag}'.", args.id))?;
    let status_label = {
        let task = &mut tag_ctx.tasks[task_idx];
        task.status = status;
        task.updated_at = Some(now_timestamp());
        task.status.clone()
    };
    save_project(&ctx.paths, &load.project)?;
    println!("Updated task [{}] to {}.", args.id, status_label);
    Ok(())
}

fn next_task(ctx: &TaskContext, args: &TaskNextArgs) -> Result<()> {
    let load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let task = tag_ctx.tasks.iter().find(|task| !task.status.is_done());
    match task {
        Some(task) => {
            if args.json {
                println!("{}", serde_json::to_string_pretty(task)?);
            } else {
                println!("[{}] {}", task.id, task.title);
            }
        }
        None => println!("No pending tasks in tag '{tag}'."),
    }
    Ok(())
}

fn search_tasks(ctx: &TaskContext, args: &TaskSearchArgs) -> Result<()> {
    let list_args = TaskListArgs {
        tag: args.tag.clone(),
        status: None,
        search: Some(args.query.clone()),
        json: args.json,
        all_tags: false,
    };
    list_tasks(ctx, &list_args)
}

fn move_task(ctx: &TaskContext, args: &TaskMoveArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    let from_tag = ctx.resolve_tag(args.from.as_deref());
    let to_tag = args.to.trim();
    if from_tag == to_tag {
        println!("Task [{}] already in tag '{to_tag}'.", args.id);
        return Ok(());
    }

    let task = {
        let from_ctx = load
            .project
            .tags
            .get_mut(&from_tag)
            .ok_or_else(|| anyhow::anyhow!("No tag named '{from_tag}' found."))?;
        let idx = find_task_index(from_ctx, &args.id)
            .ok_or_else(|| anyhow::anyhow!("Task [{}] not found in tag '{from_tag}'.", args.id))?;
        from_ctx.tasks.remove(idx)
    };

    let to_ctx = ensure_tag(&mut load.project, to_tag);
    to_ctx.tasks.push(task);
    save_project(&ctx.paths, &load.project)?;
    println!("Moved task [{}] from '{from_tag}' to '{to_tag}'.", args.id);
    Ok(())
}

fn toggle_agent(ctx: &TaskContext, args: &TaskAgentArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let task_idx = find_task_index(tag_ctx, &args.id)
        .ok_or_else(|| anyhow::anyhow!("Task [{}] not found in tag '{tag}'.", args.id))?;
    let active_agent = {
        let task = &mut tag_ctx.tasks[task_idx];
        if args.on {
            task.active_agent = true;
        } else if args.off {
            task.active_agent = false;
        } else if args.toggle || (!args.on && !args.off) {
            task.active_agent = !task.active_agent;
        }
        task.updated_at = Some(now_timestamp());
        task.active_agent
    };
    save_project(&ctx.paths, &load.project)?;
    println!("Task [{}] active_agent: {}", args.id, active_agent);
    Ok(())
}

fn list_tags(ctx: &TaskContext, args: &TagListArgs) -> Result<()> {
    let load = load_project(&ctx.paths)?;
    let mut tags: Vec<_> = load.project.tags.keys().cloned().collect();
    tags.sort();
    if args.json {
        println!("{}", serde_json::to_string_pretty(&tags)?);
        return Ok(());
    }
    for tag in tags {
        if let Some(ctx) = load.project.tags.get(&tag) {
            println!("{tag} ({})", ctx.tasks.len());
        }
    }
    Ok(())
}

fn add_tag(ctx: &TaskContext, args: &TagAddArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    if load.project.tags.contains_key(&args.name) {
        bail!("Tag '{}' already exists.", args.name);
    }
    ensure_tag(&mut load.project, &args.name);
    save_project(&ctx.paths, &load.project)?;
    println!("Added tag '{}'", args.name);
    Ok(())
}

fn rename_tag(ctx: &TaskContext, args: &TagRenameArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    if !load.project.tags.contains_key(&args.from) {
        bail!("Tag '{}' not found.", args.from);
    }
    if load.project.tags.contains_key(&args.to) {
        bail!("Tag '{}' already exists.", args.to);
    }
    if let Some(ctx) = load.project.tags.remove(&args.from) {
        load.project.tags.insert(args.to.clone(), ctx);
    }
    save_project(&ctx.paths, &load.project)?;
    println!("Renamed tag '{}' -> '{}'.", args.from, args.to);
    Ok(())
}

fn remove_tag(ctx: &TaskContext, args: &TagRemoveArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    if load.project.tags.remove(&args.name).is_none() {
        bail!("Tag '{}' not found.", args.name);
    }
    save_project(&ctx.paths, &load.project)?;
    println!("Removed tag '{}'", args.name);
    Ok(())
}

fn set_tag(ctx: &TaskContext, args: &TagSetArgs) -> Result<()> {
    touch_state(&ctx.paths, Some(&args.name))?;
    println!("Current tag set to '{}'", args.name);
    Ok(())
}

fn current_tag(ctx: &TaskContext, args: &TagCurrentArgs) -> Result<()> {
    let tag = ctx.resolve_tag(None);
    let ProjectLoad { project, exists } = load_project(&ctx.paths)?;
    let task_count = if exists {
        project
            .tags
            .get(&tag)
            .map(|tag_ctx| tag_ctx.tasks.len())
            .unwrap_or(0)
    } else {
        0
    };

    if args.json {
        let payload = json!({
            "tag": tag,
            "task_count": task_count,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("{}", tag);
    }

    Ok(())
}

fn show_tag_prd(ctx: &TaskContext, args: &TagPrdShowArgs) -> Result<()> {
    let load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let prd = read_tag_prd(tag_ctx, &tag)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&prd)?);
        return Ok(());
    }

    if let Some(prd) = prd {
        let resolved = resolve_user_path(&ctx.paths.root, &prd.path);
        println!("Tag '{}' PRD: {}", tag, prd.path);
        println!("Resolved: {}", resolved.display());
        println!("Exists: {}", if resolved.exists() { "yes" } else { "no" });
    } else {
        println!("Tag '{}' has no PRD link.", tag);
    }

    Ok(())
}

fn set_tag_prd(ctx: &TaskContext, args: &TagPrdSetArgs) -> Result<()> {
    if args.path.trim().is_empty() {
        bail!("PRD path cannot be empty.");
    }

    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;

    let resolved = resolve_user_path(&ctx.paths.root, &args.path);
    let stored_path = normalize_prd_path(&ctx.paths.root, &resolved);
    tag_ctx.set_tag_prd(TaskPrd {
        path: stored_path,
        updated_at: Some(now_timestamp()),
        version: Some(1),
        extra: HashMap::new(),
    });

    save_project(&ctx.paths, &load.project)?;
    println!("Linked tag '{}' to PRD {}", tag, resolved.display());
    Ok(())
}

fn clear_tag_prd(ctx: &TaskContext, args: &TagPrdClearArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;

    tag_ctx.clear_tag_prd();
    save_project(&ctx.paths, &load.project)?;
    println!("Cleared PRD link for tag '{}'.", tag);
    Ok(())
}

fn init_tag_prd(ctx: &TaskContext, args: &TagPrdInitArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;

    let target_abs = if let Some(path) = &args.path {
        resolve_path_arg(&ctx.paths.root, path)
    } else {
        default_tag_prd_path(&ctx.paths.root, &tag)
    };

    if target_abs.exists() && !args.force {
        bail!(
            "PRD already exists at {} (use --force to overwrite)",
            target_abs.display()
        );
    }
    if let Some(parent) = target_abs.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let content = render_tag_prd_template(&tag, &tag_ctx.tasks);
    fs::write(&target_abs, content)
        .with_context(|| format!("Failed to write {}", target_abs.display()))?;

    let stored_path = normalize_prd_path(&ctx.paths.root, &target_abs);
    tag_ctx.set_tag_prd(TaskPrd {
        path: stored_path,
        updated_at: Some(now_timestamp()),
        version: Some(1),
        extra: HashMap::new(),
    });
    save_project(&ctx.paths, &load.project)?;
    println!(
        "Initialized PRD for tag '{}' at {}",
        tag,
        target_abs.display()
    );
    Ok(())
}

fn read_tag_prd(tag_ctx: &TagContext, tag: &str) -> Result<Option<TaskPrd>> {
    let Some(raw) = tag_ctx.extra.get(TAG_PRD_KEY) else {
        return Ok(None);
    };

    let prd: TaskPrd = serde_json::from_value(raw.clone()).with_context(|| {
        format!(
            "Tag '{}' has malformed {} payload in tasks.json",
            tag, TAG_PRD_KEY
        )
    })?;
    if prd.path.trim().is_empty() {
        bail!("Tag '{}' has empty {}.path", tag, TAG_PRD_KEY);
    }
    Ok(Some(prd))
}

fn show_task_prd(ctx: &TaskContext, args: &PrdShowArgs) -> Result<()> {
    let load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let task = tag_ctx
        .tasks
        .iter()
        .find(|task| task.id == args.id)
        .ok_or_else(|| anyhow::anyhow!("Task [{}] not found in tag '{tag}'.", args.id))?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&task.aoc_prd)?);
        return Ok(());
    }

    if let Some(prd) = &task.aoc_prd {
        let resolved = resolve_user_path(&ctx.paths.root, &prd.path);
        println!("Task [{}] PRD: {}", task.id, prd.path);
        println!("Resolved: {}", resolved.display());
        println!("Exists: {}", if resolved.exists() { "yes" } else { "no" });
    } else {
        println!("Task [{}] has no task-level PRD link.", task.id);
        if let Some(tag_prd) = read_tag_prd(tag_ctx, &tag)? {
            let resolved = resolve_user_path(&ctx.paths.root, &tag_prd.path);
            println!("Tag fallback ({}): {}", tag, tag_prd.path);
            println!("Resolved: {}", resolved.display());
            println!("Exists: {}", if resolved.exists() { "yes" } else { "no" });
        }
    }
    Ok(())
}

fn set_task_prd(ctx: &TaskContext, args: &PrdSetArgs) -> Result<()> {
    if args.path.trim().is_empty() {
        bail!("PRD path cannot be empty.");
    }

    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let task_idx = find_task_index(tag_ctx, &args.id)
        .ok_or_else(|| anyhow::anyhow!("Task [{}] not found in tag '{tag}'.", args.id))?;

    let resolved = resolve_user_path(&ctx.paths.root, &args.path);
    let stored_path = normalize_prd_path(&ctx.paths.root, &resolved);
    let task_id = {
        let task = &mut tag_ctx.tasks[task_idx];
        task.aoc_prd = Some(TaskPrd {
            path: stored_path,
            updated_at: Some(now_timestamp()),
            version: Some(1),
            extra: HashMap::new(),
        });
        task.updated_at = Some(now_timestamp());
        task.id.clone()
    };
    save_project(&ctx.paths, &load.project)?;
    println!("Linked task [{}] to PRD {}", task_id, resolved.display());
    Ok(())
}

fn clear_task_prd(ctx: &TaskContext, args: &PrdClearArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let task_idx = find_task_index(tag_ctx, &args.id)
        .ok_or_else(|| anyhow::anyhow!("Task [{}] not found in tag '{tag}'.", args.id))?;
    let task_id = {
        let task = &mut tag_ctx.tasks[task_idx];
        task.aoc_prd = None;
        task.updated_at = Some(now_timestamp());
        task.id.clone()
    };
    save_project(&ctx.paths, &load.project)?;
    println!("Cleared PRD link for task [{}].", task_id);
    Ok(())
}

fn init_task_prd(ctx: &TaskContext, args: &PrdInitArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let task_idx = find_task_index(tag_ctx, &args.id)
        .ok_or_else(|| anyhow::anyhow!("Task [{}] not found in tag '{tag}'.", args.id))?;

    let target_abs = {
        let task = &tag_ctx.tasks[task_idx];
        if let Some(path) = &args.path {
            resolve_path_arg(&ctx.paths.root, path)
        } else {
            default_prd_path(&ctx.paths.root, task)
        }
    };

    if target_abs.exists() && !args.force {
        bail!(
            "PRD already exists at {} (use --force to overwrite)",
            target_abs.display()
        );
    }
    if let Some(parent) = target_abs.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    {
        let task = &tag_ctx.tasks[task_idx];
        let content = render_prd_template(task, &tag);
        fs::write(&target_abs, content)
            .with_context(|| format!("Failed to write {}", target_abs.display()))?;
    }

    let stored_path = normalize_prd_path(&ctx.paths.root, &target_abs);
    let task_id = {
        let task = &mut tag_ctx.tasks[task_idx];
        task.aoc_prd = Some(TaskPrd {
            path: stored_path,
            updated_at: Some(now_timestamp()),
            version: Some(1),
            extra: HashMap::new(),
        });
        task.updated_at = Some(now_timestamp());
        task.id.clone()
    };
    save_project(&ctx.paths, &load.project)?;
    println!(
        "Initialized PRD for task [{}] at {}",
        task_id,
        target_abs.display()
    );
    Ok(())
}

fn resolve_path_arg(root: &Path, path: &PathBuf) -> PathBuf {
    resolve_user_path(root, &path.to_string_lossy())
}

fn resolve_user_path(root: &Path, path: &str) -> PathBuf {
    if let Some(expanded) = expand_tilde(path) {
        return expanded;
    }
    let raw = PathBuf::from(path);
    if raw.is_absolute() {
        raw
    } else {
        root.join(raw)
    }
}

fn normalize_prd_path(root: &Path, abs_path: &Path) -> String {
    if let Ok(relative) = abs_path.strip_prefix(root) {
        return relative.to_string_lossy().to_string();
    }
    abs_path.to_string_lossy().to_string()
}

fn default_tag_prd_path(root: &Path, tag: &str) -> PathBuf {
    let slug = slugify(tag);
    let file = if slug.is_empty() {
        "tag-prd.md".to_string()
    } else {
        format!("tag-{}-prd.md", slug)
    };
    root.join(".taskmaster")
        .join("docs")
        .join("prds")
        .join(file)
}

fn default_prd_path(root: &Path, task: &Task) -> PathBuf {
    let slug = slugify(&task.title);
    let file = if slug.is_empty() {
        format!("{}.md", task.id)
    } else {
        format!("{}-{}.md", task.id, slug)
    };
    root.join(".taskmaster")
        .join("docs")
        .join("prds")
        .join(file)
}

fn render_tag_prd_template(tag: &str, tasks: &[Task]) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Tag PRD: {}\n\n", tag));
    out.push_str("## Metadata\n");
    out.push_str(&format!("- Tag: {}\n", tag));
    out.push_str(&format!("- Task Count: {}\n\n", tasks.len()));
    out.push_str("## Problem\n");
    out.push_str("Describe the product/problem scope for this tag/workstream.\n\n");
    out.push_str("## Goals\n- \n\n");
    out.push_str("## Non-Goals\n- \n\n");
    out.push_str("## Requirements\n- \n\n");
    out.push_str("## Acceptance Criteria\n- [ ] \n\n");
    out.push_str("## Test Strategy\n- Define validation commands and expected outcomes.\n\n");
    out.push_str("## Related Tasks\n");
    if tasks.is_empty() {
        out.push_str("- (no tasks yet)\n");
    } else {
        for task in tasks {
            out.push_str(&format!("- [{}] {}\n", task.id, task.title));
        }
    }
    out
}

fn render_prd_template(task: &Task, tag: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# PRD: {}\n\n", task.title));
    out.push_str("## Metadata\n");
    out.push_str(&format!("- Task ID: {}\n", task.id));
    out.push_str(&format!("- Tag: {}\n", tag));
    out.push_str(&format!("- Status: {}\n", task.status));
    out.push_str(&format!("- Priority: {}\n\n", task.priority));
    out.push_str("## Problem\n");
    if task.description.trim().is_empty() {
        out.push_str("Describe the problem this task solves.\n\n");
    } else {
        out.push_str(task.description.trim());
        out.push_str("\n\n");
    }
    out.push_str("## Goals\n- \n\n");
    out.push_str("## Non-Goals\n- \n\n");
    out.push_str("## Requirements\n- \n\n");
    out.push_str("## Acceptance Criteria\n- [ ] \n\n");
    out.push_str("## Test Strategy\n");
    if task.test_strategy.trim().is_empty() {
        out.push_str("- Define validation commands and expected outcomes.\n");
    } else {
        out.push_str(task.test_strategy.trim());
        out.push('\n');
    }
    out
}

fn add_subtask(ctx: &TaskContext, args: &SubAddArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let task_idx = find_task_index(tag_ctx, &args.task_id)
        .ok_or_else(|| anyhow::anyhow!("Task [{}] not found in tag '{tag}'.", args.task_id))?;
    let task = &mut tag_ctx.tasks[task_idx];
    let sub_id = next_subtask_id(task);
    let subtask = Subtask {
        id: sub_id,
        title: args.title.clone(),
        description: args.desc.clone().unwrap_or_default(),
        status: args.status.clone().unwrap_or_default(),
        dependencies: args.depends.clone(),
        extra: HashMap::new(),
    };
    task.subtasks.push(subtask);
    task.updated_at = Some(now_timestamp());
    save_project(&ctx.paths, &load.project)?;
    println!("Added subtask [{}] to task [{}]", sub_id, args.task_id);
    Ok(())
}

fn edit_subtask(ctx: &TaskContext, args: &SubEditArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let task_idx = find_task_index(tag_ctx, &args.task_id)
        .ok_or_else(|| anyhow::anyhow!("Task [{}] not found in tag '{tag}'.", args.task_id))?;
    let task = &mut tag_ctx.tasks[task_idx];
    let sub_idx = find_subtask_index(task, args.sub_id).ok_or_else(|| {
        anyhow::anyhow!(
            "Subtask [{}] not found in task [{}].",
            args.sub_id,
            args.task_id
        )
    })?;
    let subtask = &mut task.subtasks[sub_idx];
    if let Some(title) = &args.title {
        subtask.title = title.clone();
    }
    if let Some(desc) = &args.desc {
        subtask.description = desc.clone();
    }
    if let Some(status) = args.status.clone() {
        subtask.status = status;
    }
    if args.clear_deps {
        subtask.dependencies.clear();
    } else if !args.depends.is_empty() {
        subtask.dependencies = args.depends.clone();
    }
    task.updated_at = Some(now_timestamp());
    save_project(&ctx.paths, &load.project)?;
    println!(
        "Updated subtask [{}] in task [{}]",
        args.sub_id, args.task_id
    );
    Ok(())
}

fn remove_subtask(ctx: &TaskContext, args: &SubRemoveArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let task_idx = find_task_index(tag_ctx, &args.task_id)
        .ok_or_else(|| anyhow::anyhow!("Task [{}] not found in tag '{tag}'.", args.task_id))?;
    let task = &mut tag_ctx.tasks[task_idx];
    let sub_idx = find_subtask_index(task, args.sub_id).ok_or_else(|| {
        anyhow::anyhow!(
            "Subtask [{}] not found in task [{}].",
            args.sub_id,
            args.task_id
        )
    })?;
    task.subtasks.remove(sub_idx);
    task.updated_at = Some(now_timestamp());
    save_project(&ctx.paths, &load.project)?;
    println!(
        "Removed subtask [{}] from task [{}]",
        args.sub_id, args.task_id
    );
    Ok(())
}

fn set_subtask_status(ctx: &TaskContext, args: &SubTargetArgs, status: TaskStatus) -> Result<()> {
    let sub_args = SubStatusArgs {
        task_id: args.task_id.clone(),
        sub_id: args.sub_id,
        status,
        tag: args.tag.clone(),
    };
    set_subtask_status_explicit(ctx, &sub_args)
}

fn set_subtask_status_explicit(ctx: &TaskContext, args: &SubStatusArgs) -> Result<()> {
    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;
    let task_idx = find_task_index(tag_ctx, &args.task_id)
        .ok_or_else(|| anyhow::anyhow!("Task [{}] not found in tag '{tag}'.", args.task_id))?;
    let task = &mut tag_ctx.tasks[task_idx];
    let sub_idx = find_subtask_index(task, args.sub_id).ok_or_else(|| {
        anyhow::anyhow!(
            "Subtask [{}] not found in task [{}].",
            args.sub_id,
            args.task_id
        )
    })?;
    task.subtasks[sub_idx].status = args.status.clone();
    task.updated_at = Some(now_timestamp());
    save_project(&ctx.paths, &load.project)?;
    println!(
        "Updated subtask [{}] in task [{}] to {}",
        args.sub_id, args.task_id, args.status
    );
    Ok(())
}

fn sync_tasks(ctx: &TaskContext, args: &TaskSyncArgs) -> Result<()> {
    if args.from.is_none() && args.to.is_none() {
        bail!("Specify --from or --to for sync.");
    }

    if let Some(source) = args.from {
        match source {
            SyncSource::Claude => sync_from_claude(ctx, args)?,
        }
    }

    if let Some(source) = args.to {
        match source {
            SyncSource::Claude => sync_to_claude(ctx, args)?,
        }
    }

    Ok(())
}

fn sync_from_claude(ctx: &TaskContext, args: &TaskSyncArgs) -> Result<()> {
    let plans_dir = resolve_claude_plans_dir(ctx, args.path.as_ref())?;
    if !plans_dir.exists() {
        bail!("Claude plans directory not found: {}", plans_dir.display());
    }

    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let mut next_id = next_task_id(&load.project).parse::<u64>().unwrap_or(1);
    let tag_ctx = ensure_tag(&mut load.project, &tag);
    let plan_files = collect_plan_files(&plans_dir)?;

    if plan_files.is_empty() {
        println!("No plan files found in {}", plans_dir.display());
        return Ok(());
    }

    let mut imported = 0usize;

    for plan_path in plan_files {
        let plan_key = canonicalize_display(&plan_path);
        if task_has_plan_file(tag_ctx, &plan_key) {
            continue;
        }

        let content = fs::read_to_string(&plan_path)
            .with_context(|| format!("Failed to read {}", plan_path.display()))?;
        let filename = plan_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Claude Plan");
        let parsed = parse_plan(&content, filename);

        if let Some(existing_id) = parsed.metadata.get("aoc-task-id") {
            if tag_ctx.tasks.iter().any(|task| task.id == *existing_id) {
                continue;
            }
        }

        if args.dry_run {
            println!(
                "Would import plan {} -> {}",
                plan_path.display(),
                parsed.title
            );
            imported += 1;
            continue;
        }

        let id = next_id.to_string();
        next_id += 1;
        let mut extra = HashMap::new();
        extra.insert("claudePlanFile".to_string(), Value::String(plan_key));
        if let Some(plan_id) = parsed.metadata.get("aoc-task-id") {
            extra.insert("claudePlanId".to_string(), Value::String(plan_id.clone()));
        }
        let task = Task {
            id: id.clone(),
            title: parsed.title,
            description: parsed.description,
            details: content,
            test_strategy: String::new(),
            status: TaskStatus::Pending,
            dependencies: Vec::new(),
            priority: ctx.resolve_priority(None),
            subtasks: parsed.subtasks,
            aoc_prd: None,
            updated_at: Some(now_timestamp()),
            active_agent: false,
            extra,
        };
        tag_ctx.tasks.push(task);
        imported += 1;
    }

    if args.dry_run {
        println!("Plans detected for import: {}", imported);
        return Ok(());
    }

    if imported > 0 {
        save_project(&ctx.paths, &load.project)?;
        println!("Imported {} plan(s) into tag '{tag}'.", imported);
    } else {
        println!("No new plans to import.");
    }

    Ok(())
}

fn sync_to_claude(ctx: &TaskContext, args: &TaskSyncArgs) -> Result<()> {
    let plans_dir = resolve_claude_plans_dir(ctx, args.path.as_ref())?;
    if !plans_dir.exists() {
        if args.dry_run {
            println!(
                "Would create Claude plans directory: {}",
                plans_dir.display()
            );
        } else {
            fs::create_dir_all(&plans_dir)
                .with_context(|| format!("Failed to create {}", plans_dir.display()))?;
        }
    }

    let mut load = load_project(&ctx.paths)?;
    let tag = ctx.resolve_tag(args.tag.as_deref());
    let tag_ctx = load
        .project
        .tags
        .get_mut(&tag)
        .ok_or_else(|| anyhow::anyhow!("No tag named '{tag}' found."))?;

    let mut exported = 0usize;

    for task in &mut tag_ctx.tasks {
        let filename = format_plan_filename(task);
        let plan_path = plans_dir.join(filename);
        let plan_key = canonicalize_display(&plan_path);

        if plan_path.exists() && !args.force {
            continue;
        }

        if args.dry_run {
            println!("Would export task [{}] -> {}", task.id, plan_path.display());
            exported += 1;
            continue;
        }

        let content = format_plan(task, &tag);
        fs::write(&plan_path, content)
            .with_context(|| format!("Failed to write {}", plan_path.display()))?;
        task.extra
            .insert("claudePlanFile".to_string(), Value::String(plan_key));
        exported += 1;
    }

    if args.dry_run {
        println!("Plans prepared for export: {}", exported);
        return Ok(());
    }

    if exported > 0 {
        save_project(&ctx.paths, &load.project)?;
        println!("Exported {} task(s) to {}", exported, plans_dir.display());
    } else {
        println!("No tasks exported.");
    }

    Ok(())
}

fn resolve_claude_plans_dir(ctx: &TaskContext, override_path: Option<&PathBuf>) -> Result<PathBuf> {
    if let Some(path) = override_path {
        return Ok(resolve_plans_path(&ctx.paths.root, path));
    }

    if let Ok(path) = std::env::var("AOC_CLAUDE_PLANS_DIR") {
        if !path.trim().is_empty() {
            return Ok(resolve_plans_path_str(&ctx.paths.root, &path));
        }
    }

    if let Some(path) = find_claude_plans_setting(&ctx.paths.root) {
        return Ok(resolve_plans_path_str(&ctx.paths.root, &path));
    }

    Ok(resolve_plans_path_str(&ctx.paths.root, "~/.claude/plans"))
}

fn resolve_plans_path(root: &Path, path: &PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path.clone();
    }
    if let Some(path_str) = path.to_str() {
        if let Some(expanded) = expand_tilde(path_str) {
            return expanded;
        }
    }
    root.join(path)
}

fn resolve_plans_path_str(root: &Path, path: &str) -> PathBuf {
    if let Some(expanded) = expand_tilde(path) {
        return expanded;
    }
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn expand_tilde(path: &str) -> Option<PathBuf> {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return Some(home.join(stripped));
        }
    }
    if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return Some(home);
        }
    }
    None
}

fn find_claude_plans_setting(root: &Path) -> Option<String> {
    let candidates = [
        root.join(".claude/settings.local.json"),
        root.join(".claude/settings.json"),
        dirs::home_dir()?.join(".claude/settings.json"),
    ];

    for path in candidates {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<ClaudeSettings>(&content) {
                if let Some(dir) = settings.plans_directory {
                    if !dir.trim().is_empty() {
                        return Some(dir);
                    }
                }
            }
        }
    }
    None
}

fn collect_plan_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn canonicalize_display(path: &Path) -> String {
    if let Ok(abs) = fs::canonicalize(path) {
        return abs.to_string_lossy().to_string();
    }
    path.to_string_lossy().to_string()
}

fn task_has_plan_file(ctx: &TagContext, plan_key: &str) -> bool {
    ctx.tasks.iter().any(|task| {
        task.extra
            .get("claudePlanFile")
            .and_then(|value| value.as_str())
            .map(|value| value == plan_key)
            .unwrap_or(false)
    })
}

struct ParsedPlan {
    title: String,
    description: String,
    subtasks: Vec<Subtask>,
    metadata: HashMap<String, String>,
}

fn parse_plan(content: &str, fallback_title: &str) -> ParsedPlan {
    let lines: Vec<&str> = content.lines().collect();
    let metadata = parse_plan_metadata(&lines);
    let title = lines
        .iter()
        .find_map(|line| {
            let trimmed = line.trim_start();
            trimmed
                .strip_prefix("# ")
                .map(|title| title.trim().to_string())
        })
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| fallback_title.to_string());
    let description = extract_description(&lines)
        .unwrap_or_else(|| format!("Imported from Claude plan: {fallback_title}"));
    let subtasks = extract_subtasks(&lines);

    ParsedPlan {
        title,
        description,
        subtasks,
        metadata,
    }
}

fn parse_plan_metadata(lines: &[&str]) -> HashMap<String, String> {
    let mut meta = HashMap::new();
    for line in lines {
        let trimmed = line.trim();
        if let Some(comment) = trimmed
            .strip_prefix("<!--")
            .and_then(|c| c.strip_suffix("-->"))
        {
            let comment = comment.trim();
            if let Some(value) = comment.strip_prefix("aoc-task-id:") {
                meta.insert("aoc-task-id".to_string(), value.trim().to_string());
            }
            if let Some(value) = comment.strip_prefix("aoc-task-tag:") {
                meta.insert("aoc-task-tag".to_string(), value.trim().to_string());
            }
        }
    }
    meta
}

fn extract_description(lines: &[&str]) -> Option<String> {
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("#") {
            continue;
        }
        if trimmed.starts_with("<!--") {
            continue;
        }
        if parse_checkbox(trimmed).is_some() {
            continue;
        }
        if trimmed.starts_with('-') || trimmed.starts_with('*') {
            continue;
        }
        return Some(trimmed.to_string());
    }
    None
}

fn extract_subtasks(lines: &[&str]) -> Vec<Subtask> {
    let mut subtasks = Vec::new();
    let mut next_id = 1u32;
    for line in lines {
        if let Some((done, title)) = parse_checkbox(line) {
            let status = if done {
                TaskStatus::Done
            } else {
                TaskStatus::Pending
            };
            subtasks.push(Subtask {
                id: next_id,
                title,
                description: String::new(),
                status,
                dependencies: Vec::new(),
                extra: HashMap::new(),
            });
            next_id += 1;
        }
    }
    subtasks
}

fn parse_checkbox(line: &str) -> Option<(bool, String)> {
    let trimmed = line.trim_start();
    let trimmed = trimmed
        .strip_prefix("- [")
        .or_else(|| trimmed.strip_prefix("* ["))?;
    let mut chars = trimmed.chars();
    let marker = chars.next()?;
    let closing = chars.next()?;
    if closing != ']' {
        return None;
    }
    let title = chars.as_str().trim().to_string();
    if title.is_empty() {
        return None;
    }
    let done = matches!(marker, 'x' | 'X');
    Some((done, title))
}

fn format_plan(task: &Task, tag: &str) -> String {
    let mut output = String::new();
    output.push_str(&format!("<!-- aoc-task-id: {} -->\n", task.id));
    output.push_str(&format!("<!-- aoc-task-tag: {} -->\n\n", tag));
    output.push_str(&format!("# {}\n\n", task.title));
    output.push_str(&format!("Status: {}\n", task.status));
    output.push_str(&format!("Priority: {}\n\n", task.priority));

    if !task.description.is_empty() {
        output.push_str("## Description\n");
        output.push_str(task.description.trim());
        output.push_str("\n\n");
    }
    if !task.details.is_empty() {
        output.push_str("## Details\n");
        output.push_str(task.details.trim());
        output.push_str("\n\n");
    }

    if !task.subtasks.is_empty() {
        output.push_str("## Subtasks\n");
        for sub in &task.subtasks {
            let mark = if sub.status.is_done() { "x" } else { " " };
            output.push_str(&format!("- [{}] {}\n", mark, sub.title));
        }
    }

    output
}

fn format_plan_filename(task: &Task) -> String {
    let slug = slugify(&task.title);
    if slug.is_empty() {
        format!("{}.md", task.id)
    } else {
        format!("{}-{}.md", task.id, slug)
    }
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeSettings {
    plans_directory: Option<String>,
}

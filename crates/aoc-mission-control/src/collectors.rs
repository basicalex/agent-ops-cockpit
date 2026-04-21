//! Local observation, layout, proc, git, and health collectors.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

pub(crate) fn collect_local(config: &Config) -> LocalSnapshot {
    collect_local_with_options(config, true, true, true, None)
}

pub(crate) fn collect_local_with_options(
    config: &Config,
    include_work: bool,
    include_diff: bool,
    include_health: bool,
    previous: Option<&LocalSnapshot>,
) -> LocalSnapshot {
    let session_layout = collect_session_layout(&config.session_id);
    let viewer_tab_index = collect_viewer_tab_index(config, session_layout.as_ref());
    let tab_roster = session_layout
        .as_ref()
        .map(|layout| layout.tabs.clone())
        .or_else(|| previous.map(|snapshot| snapshot.tab_roster.clone()))
        .unwrap_or_default();
    let mut overview = collect_runtime_overview(config, session_layout.as_ref());
    if overview.is_empty() {
        overview = collect_proc_overview(config, session_layout.as_ref());
    }
    let project_roots = collect_project_roots(&overview, &config.project_root);
    let (work, taskmaster_status) = if include_work || include_health {
        collect_local_work(&project_roots)
    } else {
        (
            previous
                .map(|snapshot| snapshot.work.clone())
                .unwrap_or_default(),
            previous
                .map(|snapshot| snapshot.health.taskmaster_status.clone())
                .unwrap_or_else(|| "unknown".to_string()),
        )
    };
    let diff = if include_diff {
        collect_local_diff(&project_roots)
    } else {
        previous
            .map(|snapshot| snapshot.diff.clone())
            .unwrap_or_default()
    };
    let health = if include_health {
        collect_health(config, &taskmaster_status)
    } else {
        previous
            .map(|snapshot| snapshot.health.clone())
            .unwrap_or(HealthSnapshot {
                dependencies: Vec::new(),
                checks: Vec::new(),
                taskmaster_status,
            })
    };
    LocalSnapshot {
        overview,
        viewer_tab_index,
        tab_roster,
        work,
        diff,
        health,
    }
}

pub(crate) fn collect_layout_overview(
    config: &Config,
    existing_rows: &[OverviewRow],
    tab_cache: &HashMap<String, TabMeta>,
) -> (Vec<OverviewRow>, Option<usize>, Vec<TabMeta>) {
    let session_layout = collect_session_layout(&config.session_id);
    let viewer_tab_index = collect_viewer_tab_index(config, session_layout.as_ref());
    let tab_roster = session_layout
        .as_ref()
        .map(|layout| layout.tabs.clone())
        .unwrap_or_default();
    if existing_rows.is_empty() {
        return (Vec::new(), viewer_tab_index, tab_roster);
    }
    let Some(layout) = session_layout.as_ref() else {
        let mut rows = existing_rows.to_vec();
        for row in &mut rows {
            apply_cached_tab_meta(row, tab_cache);
        }
        return (sort_overview_rows(rows), viewer_tab_index, tab_roster);
    };

    let mut rows = existing_rows.to_vec();
    for row in &mut rows {
        if let Some(meta) = layout
            .pane_tabs
            .get(&row.pane_id)
            .or_else(|| layout.project_tabs.get(&row.project_root))
        {
            row.tab_index = Some(meta.index);
            row.tab_name = Some(meta.name.clone());
            row.tab_focused = meta.focused;
        } else {
            row.tab_focused = false;
            apply_cached_tab_meta(row, tab_cache);
        }
    }
    (sort_overview_rows(rows), viewer_tab_index, tab_roster)
}

pub(crate) fn collect_viewer_tab_index(
    config: &Config,
    session_layout: Option<&SessionLayout>,
) -> Option<usize> {
    if config.pane_id.trim().is_empty() {
        return None;
    }
    session_layout
        .and_then(|layout| layout.pane_tabs.get(&config.pane_id))
        .map(|meta| meta.index)
}

pub(crate) fn hub_layout_from_payload(payload: &LayoutStatePayload) -> HubLayout {
    let mut pane_tabs = HashMap::new();
    for pane in &payload.panes {
        let Ok(index) = usize::try_from(pane.tab_index) else {
            continue;
        };
        pane_tabs.insert(
            pane.pane_id.clone(),
            TabMeta {
                index,
                name: pane.tab_name.clone(),
                focused: pane.tab_focused,
            },
        );
    }

    let focused_tab_index = payload
        .tabs
        .iter()
        .find(|tab| tab.focused)
        .and_then(|tab| usize::try_from(tab.index).ok())
        .or_else(|| {
            payload
                .panes
                .iter()
                .find(|pane| pane.tab_focused)
                .and_then(|pane| usize::try_from(pane.tab_index).ok())
        });

    HubLayout {
        layout_seq: payload.layout_seq,
        pane_tabs,
        focused_tab_index,
    }
}

pub(crate) fn collect_runtime_overview(
    config: &Config,
    session_layout: Option<&SessionLayout>,
) -> Vec<OverviewRow> {
    let mut rows: BTreeMap<String, OverviewRow> = BTreeMap::new();
    let viewer_scope = config.tab_scope.as_deref();
    let telemetry_dir = config
        .state_dir
        .join("telemetry")
        .join(sanitize_component(&config.session_id));
    let entries = match fs::read_dir(telemetry_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let now = Utc::now();
    let active_panes = session_layout.as_ref().and_then(|layout| {
        if layout.pane_ids.is_empty() {
            None
        } else {
            Some(&layout.pane_ids)
        }
    });
    let pane_tabs = session_layout.as_ref().map(|layout| &layout.pane_tabs);
    let project_tabs = session_layout.as_ref().map(|layout| &layout.project_tabs);
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(_) => continue,
        };
        let snapshot: RuntimeSnapshot = match serde_json::from_str(&contents) {
            Ok(snapshot) => snapshot,
            Err(_) => continue,
        };
        if snapshot.session_id != config.session_id {
            continue;
        }
        if let Some(panes) = active_panes.as_ref() {
            if !panes.contains(&snapshot.pane_id) {
                continue;
            }
        }
        let heartbeat_age = DateTime::parse_from_rfc3339(&snapshot.last_update)
            .ok()
            .map(|dt| {
                now.signed_duration_since(dt.with_timezone(&Utc))
                    .num_seconds()
                    .max(0)
            });
        if !runtime_process_matches(&snapshot) {
            continue;
        }
        let online = !snapshot.status.eq_ignore_ascii_case("offline");
        let expected_identity = build_identity_key(&snapshot.session_id, &snapshot.pane_id);
        let identity_key = if snapshot.agent_id == expected_identity {
            snapshot.agent_id.clone()
        } else {
            expected_identity
        };
        let tab_meta = pane_tabs
            .and_then(|tabs| tabs.get(&snapshot.pane_id))
            .or_else(|| project_tabs.and_then(|tabs| tabs.get(&snapshot.project_root)));
        let tab_name = tab_meta
            .map(|meta| meta.name.clone())
            .or_else(|| snapshot.tab_scope.clone());
        let tab_focused = tab_scope_matches(viewer_scope, snapshot.tab_scope.as_deref())
            || tab_scope_matches(viewer_scope, tab_name.as_deref());
        let session_title = snapshot.session_title;
        rows.insert(
            identity_key.clone(),
            OverviewRow {
                identity_key,
                label: snapshot.agent_label,
                lifecycle: normalize_lifecycle(&snapshot.status),
                snippet: None,
                pane_id: snapshot.pane_id,
                tab_index: tab_meta.map(|meta| meta.index),
                tab_name,
                tab_focused,
                project_root: snapshot.project_root,
                online,
                age_secs: heartbeat_age,
                source: "runtime".to_string(),
                session_title,
                chat_title: snapshot.chat_title,
            },
        );
    }
    rows.into_values().collect()
}

pub(crate) fn collect_session_layout(session_id: &str) -> Option<SessionLayout> {
    if session_id.trim().is_empty() {
        return None;
    }
    let snapshot = query_session_snapshot(session_id).ok()??;
    let mut parsed = SessionLayout::default();
    parsed.pane_ids = snapshot.pane_ids;
    parsed.tabs = snapshot
        .tabs
        .into_iter()
        .filter_map(|tab| {
            usize::try_from(tab.index).ok().map(|index| TabMeta {
                index,
                name: tab.name,
                focused: tab.focused,
            })
        })
        .collect();
    parsed.focused_tab_index = snapshot
        .current_tab_index
        .and_then(|index| usize::try_from(index).ok())
        .or_else(|| {
            parsed
                .tabs
                .iter()
                .find(|tab| tab.focused)
                .map(|tab| tab.index)
        });
    for pane in snapshot.panes {
        parsed.pane_tabs.insert(
            pane.pane_id,
            TabMeta {
                index: pane.tab_index as usize,
                name: pane.tab_name,
                focused: pane.tab_focused,
            },
        );
    }
    for (project_root, tab) in snapshot.project_tabs {
        parsed.project_tabs.insert(
            project_root,
            TabMeta {
                index: tab.index as usize,
                name: tab.name,
                focused: tab.focused,
            },
        );
    }
    Some(parsed)
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn parse_layout_tabs(layout: &str) -> SessionLayout {
    let mut parsed = SessionLayout::default();
    let mut current_tab_index = 0usize;
    let mut current_tab_name = String::new();
    let mut current_tab_focused = false;
    let mut base_cwd: Option<String> = None;

    for line in layout.lines() {
        if base_cwd.is_none() {
            if let Some(cwd) = extract_layout_attr(line, "cwd") {
                if cwd.starts_with('/') {
                    base_cwd = Some(cwd);
                }
            }
        }

        if line_is_tab_decl(line) {
            current_tab_index += 1;
            current_tab_name = extract_layout_attr(line, "name")
                .unwrap_or_else(|| format!("tab-{current_tab_index}"));
            current_tab_focused = line.contains("focus=true") || line.contains("focus true");
            parsed.tabs.push(TabMeta {
                index: current_tab_index,
                name: current_tab_name.clone(),
                focused: current_tab_focused,
            });
            if current_tab_focused {
                parsed.focused_tab_index = Some(current_tab_index);
            }
        }

        if current_tab_index > 0
            && (line.contains("name=\"Agent [")
                || line.contains("name=\"Agent[")
                || line.contains("name=\"Agent\""))
        {
            if let Some(cwd) = extract_layout_attr(line, "cwd") {
                if let Some(project_root) = resolve_layout_cwd(base_cwd.as_deref(), &cwd) {
                    parsed.project_tabs.insert(
                        project_root,
                        TabMeta {
                            index: current_tab_index,
                            name: current_tab_name.clone(),
                            focused: current_tab_focused,
                        },
                    );
                }
            }
        }

        for pane_id in extract_pane_ids_from_layout_line(line) {
            parsed.pane_ids.insert(pane_id.clone());
            if current_tab_index > 0 {
                parsed.pane_tabs.insert(
                    pane_id,
                    TabMeta {
                        index: current_tab_index,
                        name: current_tab_name.clone(),
                        focused: current_tab_focused,
                    },
                );
            }
        }
    }
    parsed
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn resolve_layout_cwd(base_cwd: Option<&str>, cwd: &str) -> Option<String> {
    if cwd.trim().is_empty() {
        return None;
    }
    let path = PathBuf::from(cwd);
    if path.is_absolute() {
        return Some(path.to_string_lossy().to_string());
    }
    let base = base_cwd?;
    Some(PathBuf::from(base).join(path).to_string_lossy().to_string())
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn line_is_tab_decl(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("tab ") || trimmed == "tab" || trimmed.starts_with("tab\t")
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn extract_layout_attr(line: &str, attr: &str) -> Option<String> {
    let with_equals = format!("{attr}=\"");
    if let Some(start) = line.find(&with_equals) {
        let value_start = start + with_equals.len();
        let tail = &line[value_start..];
        let end = tail.find('"')?;
        let value = tail[..end].trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }

    let with_space = format!("{attr} \"");
    if let Some(start) = line.find(&with_space) {
        let value_start = start + with_space.len();
        let tail = &line[value_start..];
        let end = tail.find('"')?;
        let value = tail[..end].trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(test)]
pub(crate) fn extract_pane_ids_from_layout_line(line: &str) -> Vec<String> {
    let mut pane_ids = extract_quoted_flag_values(line, "--pane-id");
    pane_ids.extend(extract_attr_values(line, "pane_id"));
    pane_ids.extend(extract_attr_values(line, "pane-id"));
    pane_ids.sort();
    pane_ids.dedup();
    pane_ids
}

#[cfg(test)]
pub(crate) fn extract_quoted_flag_values(line: &str, flag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let parts: Vec<&str> = line.split('"').collect();
    if parts.len() < 3 {
        return out;
    }
    let mut idx = 1usize;
    while idx + 2 < parts.len() {
        if parts[idx].trim() == flag {
            let value = parts[idx + 2].trim();
            if !value.is_empty() {
                out.push(value.to_string());
            }
        }
        idx += 2;
    }
    out
}

#[cfg(test)]
pub(crate) fn extract_attr_values(line: &str, attr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let marker = format!("{attr}=\"");
    let mut cursor = line;
    while let Some(idx) = cursor.find(&marker) {
        let tail = &cursor[idx + marker.len()..];
        let Some(end_quote) = tail.find('"') else {
            break;
        };
        let value = tail[..end_quote].trim();
        if !value.is_empty() {
            out.push(value.to_string());
        }
        cursor = &tail[end_quote + 1..];
    }
    out
}

pub(crate) fn runtime_process_matches(snapshot: &RuntimeSnapshot) -> bool {
    if snapshot.pid <= 0 {
        return false;
    }
    let proc_path = PathBuf::from("/proc").join(snapshot.pid.to_string());
    if !proc_path.exists() {
        return false;
    }
    let env_map = read_proc_environ(proc_path.join("environ"));
    if env_map.is_empty() {
        return false;
    }
    let proc_session = env_map
        .get("AOC_SESSION_ID")
        .or_else(|| env_map.get("ZELLIJ_SESSION_NAME"))
        .map(|value| value.as_str())
        .unwrap_or("");
    let proc_pane = env_map
        .get("AOC_PANE_ID")
        .or_else(|| env_map.get("ZELLIJ_PANE_ID"))
        .map(|value| value.as_str())
        .unwrap_or("");
    proc_session == snapshot.session_id.as_str() && proc_pane == snapshot.pane_id.as_str()
}

pub(crate) fn collect_proc_overview(
    config: &Config,
    session_layout: Option<&SessionLayout>,
) -> Vec<OverviewRow> {
    let mut rows: BTreeMap<String, OverviewRow> = BTreeMap::new();
    let viewer_scope = config.tab_scope.as_deref();
    let pane_tabs = session_layout.map(|layout| &layout.pane_tabs);
    let project_tabs = session_layout.map(|layout| &layout.project_tabs);
    let active_panes = session_layout.map(|layout| &layout.pane_ids);
    let proc_entries = match fs::read_dir("/proc") {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    for entry in proc_entries.flatten() {
        let file_name = entry.file_name();
        let pid_str = file_name.to_string_lossy();
        if pid_str.parse::<i32>().is_err() {
            continue;
        }
        let env_map = read_proc_environ(entry.path().join("environ"));
        if env_map.is_empty() {
            continue;
        }
        let session_id = env_map
            .get("AOC_SESSION_ID")
            .or_else(|| env_map.get("ZELLIJ_SESSION_NAME"))
            .cloned();
        if session_id.as_deref() != Some(config.session_id.as_str()) {
            continue;
        }
        let pane_id = env_map
            .get("AOC_PANE_ID")
            .or_else(|| env_map.get("ZELLIJ_PANE_ID"))
            .cloned();
        let pane_id = match pane_id {
            Some(value) if !value.is_empty() => value,
            _ => continue,
        };
        if let Some(active_panes) = active_panes {
            if !active_panes.contains(&pane_id) {
                continue;
            }
        }
        let agent_label = env_map.get("AOC_AGENT_LABEL").cloned();
        let agent_id = env_map.get("AOC_AGENT_ID").cloned();
        let agent_run = env_map
            .get("AOC_AGENT_RUN")
            .and_then(|value| parse_bool_flag(value))
            .unwrap_or(false);
        let has_agent_identity = agent_label
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            || agent_id
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty());
        if !has_agent_identity && !agent_run {
            continue;
        }
        let label = agent_label
            .filter(|value| !value.trim().is_empty())
            .or_else(|| agent_id.filter(|value| !value.trim().is_empty()))
            .unwrap_or_else(|| format!("pane-{}", pane_id));
        let project_root = env_map.get("AOC_PROJECT_ROOT").cloned().unwrap_or_else(|| {
            fs::read_link(entry.path().join("cwd"))
                .ok()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "(unknown)".to_string())
        });
        let key = build_identity_key(&config.session_id, &pane_id);
        let tab_meta = pane_tabs
            .and_then(|tabs| tabs.get(&pane_id))
            .or_else(|| project_tabs.and_then(|tabs| tabs.get(&project_root)));
        let proc_tab_scope = env_map
            .get("AOC_TAB_SCOPE")
            .or_else(|| env_map.get("AOC_TAB_NAME"))
            .or_else(|| env_map.get("ZELLIJ_TAB_NAME"))
            .cloned();
        let tab_name = tab_meta
            .map(|meta| meta.name.clone())
            .or(proc_tab_scope.clone());
        let tab_focused = tab_scope_matches(viewer_scope, proc_tab_scope.as_deref())
            || tab_scope_matches(viewer_scope, tab_name.as_deref());
        rows.entry(key.clone()).or_insert(OverviewRow {
            identity_key: key,
            label,
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id,
            tab_index: tab_meta.map(|meta| meta.index),
            tab_name,
            tab_focused,
            project_root,
            online: true,
            age_secs: None,
            source: "proc".to_string(),
            session_title: None,
            chat_title: None,
        });
    }
    rows.into_values().collect()
}

pub(crate) fn collect_project_roots(overview: &[OverviewRow], fallback: &Path) -> Vec<String> {
    let mut roots = BTreeMap::new();
    for row in overview {
        if row.project_root.is_empty() || row.project_root == "(unknown)" {
            continue;
        }
        roots.insert(row.project_root.clone(), true);
    }
    roots.insert(fallback.to_string_lossy().to_string(), true);
    roots.into_keys().collect()
}

pub(crate) fn collect_local_work(project_roots: &[String]) -> (Vec<WorkProject>, String) {
    let mut projects = Vec::new();
    let mut status = "tasks.json missing".to_string();
    for root in project_roots {
        let tasks_path = PathBuf::from(root)
            .join(".taskmaster")
            .join("tasks")
            .join("tasks.json");
        let contents = match fs::read_to_string(&tasks_path) {
            Ok(contents) => contents,
            Err(_) => continue,
        };
        let parsed: ProjectData = match serde_json::from_str(&contents) {
            Ok(parsed) => parsed,
            Err(_) => {
                status = format!("tasks.json malformed at {}", tasks_path.display());
                continue;
            }
        };
        status = "tasks.json available".to_string();
        let mut tags = Vec::new();
        let mut entries: Vec<_> = parsed.tags.into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (tag, ctx) in entries {
            let mut counts = TaskCounts {
                total: ctx.tasks.len() as u32,
                ..TaskCounts::default()
            };
            let mut in_progress_titles = Vec::new();
            for task in ctx.tasks {
                match task.status {
                    TaskStatus::Pending => counts.pending += 1,
                    TaskStatus::InProgress => {
                        counts.in_progress += 1;
                        in_progress_titles.push(format!("#{} {}", task.id, task.title));
                    }
                    TaskStatus::Blocked => counts.blocked += 1,
                    TaskStatus::Done | TaskStatus::Cancelled => counts.done += 1,
                    _ => {}
                }
            }
            tags.push(WorkTagRow {
                tag,
                counts,
                in_progress_titles,
            });
        }
        projects.push(WorkProject {
            project_root: root.clone(),
            scope: "local".to_string(),
            tags,
        });
    }
    (projects, status)
}

pub(crate) fn collect_local_diff(project_roots: &[String]) -> Vec<DiffProject> {
    let mut projects: BTreeMap<String, DiffProject> = BTreeMap::new();
    for root in project_roots {
        let root_path = PathBuf::from(root);
        match git_repo_root(&root_path) {
            Ok(repo_root) => match collect_git_summary(&repo_root) {
                Ok((summary, mut files)) => {
                    if files.len() > MAX_DIFF_FILES {
                        files.truncate(MAX_DIFF_FILES);
                    }
                    let key = repo_root.to_string_lossy().to_string();
                    projects.entry(key.clone()).or_insert(DiffProject {
                        project_root: key,
                        scope: "local".to_string(),
                        git_available: true,
                        reason: None,
                        summary,
                        files,
                    });
                }
                Err(err) => {
                    projects.entry(root.clone()).or_insert(DiffProject {
                        project_root: root.clone(),
                        scope: "local".to_string(),
                        git_available: false,
                        reason: Some(err),
                        summary: DiffSummaryCounts::default(),
                        files: Vec::new(),
                    });
                }
            },
            Err(reason) => {
                projects.entry(root.clone()).or_insert(DiffProject {
                    project_root: root.clone(),
                    scope: "local".to_string(),
                    git_available: false,
                    reason: Some(reason),
                    summary: DiffSummaryCounts::default(),
                    files: Vec::new(),
                });
            }
        }
    }
    projects.into_values().collect()
}

pub(crate) fn collect_health(config: &Config, taskmaster_status: &str) -> HealthSnapshot {
    let dependencies = vec![
        dep_status("git"),
        dep_status("zellij"),
        dep_status("aoc-hub"),
        dep_status("aoc-agent-wrap-rs"),
        dep_status_any(
            "task-control",
            &["aoc-task", "tm", "aoc-taskmaster", "task-master"],
        ),
    ];
    let checks = vec![
        load_check_outcome(&config.project_root, "test"),
        load_check_outcome(&config.project_root, "lint"),
        load_check_outcome(&config.project_root, "build"),
    ];
    HealthSnapshot {
        dependencies,
        checks,
        taskmaster_status: taskmaster_status.to_string(),
    }
}

pub(crate) fn dep_status(name: &str) -> DependencyStatus {
    let path = which_cmd(name);
    DependencyStatus {
        name: name.to_string(),
        available: path.is_some(),
        path,
    }
}

pub(crate) fn dep_status_any(name: &str, candidates: &[&str]) -> DependencyStatus {
    for candidate in candidates {
        if let Some(path) = which_cmd(candidate) {
            return DependencyStatus {
                name: name.to_string(),
                available: true,
                path: Some(path),
            };
        }
    }

    DependencyStatus {
        name: name.to_string(),
        available: false,
        path: None,
    }
}

pub(crate) fn load_check_outcome(project_root: &Path, kind: &str) -> CheckOutcome {
    let base = project_root.join(".aoc").join("state");
    let json_path = base.join(format!("last-{kind}.json"));
    if let Ok(contents) = fs::read_to_string(&json_path) {
        if let Ok(value) = serde_json::from_str::<Value>(&contents) {
            let status = value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let timestamp = value
                .get("timestamp")
                .and_then(Value::as_str)
                .map(|v| v.to_string());
            let details = value
                .get("summary")
                .and_then(Value::as_str)
                .map(|v| v.to_string());
            return CheckOutcome {
                name: kind.to_string(),
                status,
                timestamp,
                details,
            };
        }
    }
    let text_path = base.join(format!("last-{kind}"));
    if let Ok(contents) = fs::read_to_string(&text_path) {
        let line = contents
            .lines()
            .next()
            .unwrap_or("unknown")
            .trim()
            .to_string();
        return CheckOutcome {
            name: kind.to_string(),
            status: line,
            timestamp: None,
            details: Some("from .aoc/state marker".to_string()),
        };
    }
    CheckOutcome {
        name: kind.to_string(),
        status: "unknown".to_string(),
        timestamp: None,
        details: Some("no check marker found".to_string()),
    }
}

pub(crate) fn git_repo_root(project_root: &Path) -> Result<PathBuf, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output();
    let output = match output {
        Ok(output) => output,
        Err(_) => return Err("git_missing".to_string()),
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.contains("not a git repository") {
            return Err("not_git_repo".to_string());
        }
        return Err("git_error".to_string());
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        return Err("git_error".to_string());
    }
    Ok(PathBuf::from(root))
}

pub(crate) fn collect_git_summary(
    repo_root: &Path,
) -> Result<(DiffSummaryCounts, Vec<DiffFile>), String> {
    let staged_raw = run_git(repo_root, &["diff", "--numstat", "--cached"])?;
    let (staged_counts, staged_map) = parse_numstat(&staged_raw);
    let unstaged_raw = run_git(repo_root, &["diff", "--numstat"])?;
    let (unstaged_counts, unstaged_map) = parse_numstat(&unstaged_raw);
    let status_raw = run_git(repo_root, &["status", "--porcelain=v1", "-u"])?;
    let status_entries = parse_status_entries(&status_raw);

    let mut files = Vec::new();
    for entry in status_entries {
        let (additions, deletions) = if entry.untracked {
            (0, 0)
        } else {
            let staged_stats = staged_map.get(&entry.path).copied().unwrap_or((0, 0));
            let unstaged_stats = unstaged_map.get(&entry.path).copied().unwrap_or((0, 0));
            if entry.staged && entry.unstaged {
                (
                    staged_stats.0 + unstaged_stats.0,
                    staged_stats.1 + unstaged_stats.1,
                )
            } else if entry.staged {
                staged_stats
            } else {
                unstaged_stats
            }
        };
        files.push(DiffFile {
            path: entry.path,
            status: entry.status,
            additions,
            deletions,
            staged: entry.staged,
            untracked: entry.untracked,
        });
    }
    let untracked = files.iter().filter(|file| file.untracked).count() as u32;
    let summary = DiffSummaryCounts {
        staged: staged_counts,
        unstaged: unstaged_counts,
        untracked: UntrackedCounts { files: untracked },
    };
    Ok((summary, files))
}

pub(crate) fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|_| "git_missing".to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub(crate) fn parse_numstat(output: &str) -> (DiffCounts, HashMap<String, (u32, u32)>) {
    let mut counts = DiffCounts::default();
    let mut map = HashMap::new();
    for line in output.lines() {
        let mut parts = line.splitn(3, '\t');
        let additions = parts.next().unwrap_or("0").parse::<u32>().unwrap_or(0);
        let deletions = parts.next().unwrap_or("0").parse::<u32>().unwrap_or(0);
        let path = parts.next().unwrap_or("");
        if path.is_empty() {
            continue;
        }
        counts.files += 1;
        counts.additions += additions;
        counts.deletions += deletions;
        map.insert(path.to_string(), (additions, deletions));
    }
    (counts, map)
}

pub(crate) fn parse_status_entries(output: &str) -> Vec<GitStatusEntry> {
    let mut entries = Vec::new();
    for line in output.lines() {
        if let Some(entry) = parse_status_line(line) {
            entries.push(entry);
        }
    }
    entries
}

pub(crate) fn parse_status_line(line: &str) -> Option<GitStatusEntry> {
    if line.len() < 3 {
        return None;
    }
    if line.starts_with("?? ") {
        return Some(GitStatusEntry {
            path: line[3..].trim().to_string(),
            status: "untracked".to_string(),
            staged: false,
            unstaged: false,
            untracked: true,
        });
    }
    let mut chars = line.chars();
    let x = chars.next()?;
    let y = chars.next()?;
    let mut path = line[3..].trim().to_string();
    if let Some((_, new_path)) = path.split_once("->") {
        path = new_path.trim().to_string();
    }
    let staged = x != ' ' && x != '?';
    let unstaged = y != ' ' && y != '?';
    let status = if matches!(x, 'A' | 'C') || matches!(y, 'A' | 'C') {
        "added"
    } else if x == 'D' || y == 'D' {
        "deleted"
    } else if x == 'R' || y == 'R' {
        "renamed"
    } else {
        "modified"
    };
    Some(GitStatusEntry {
        path,
        status: status.to_string(),
        staged,
        unstaged,
        untracked: false,
    })
}

pub(crate) fn read_proc_environ(path: PathBuf) -> HashMap<String, String> {
    let mut env_map = HashMap::new();
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return env_map,
    };
    for part in bytes.split(|byte| *byte == 0) {
        if part.is_empty() {
            continue;
        }
        if let Ok(item) = String::from_utf8(part.to_vec()) {
            if let Some((key, value)) = item.split_once('=') {
                env_map.insert(key.to_string(), value.to_string());
            }
        }
    }
    env_map
}

pub(crate) fn short_status(status: &str) -> &'static str {
    match status {
        "added" => "A",
        "deleted" => "D",
        "renamed" => "R",
        "untracked" => "?",
        _ => "M",
    }
}

pub(crate) fn extract_label(identity_key: &str) -> String {
    if let Some((_, pane)) = identity_key.split_once("::") {
        return format!("pane-{pane}");
    }
    identity_key.to_string()
}

pub(crate) fn extract_pane_id(identity_key: &str) -> String {
    identity_key
        .split_once("::")
        .map(|(_, pane)| pane.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) fn build_identity_key(session_id: &str, pane_id: &str) -> String {
    format!("{session_id}::{pane_id}")
}

pub(crate) fn which_cmd(name: &str) -> Option<String> {
    let path_var = std::env::var("PATH").ok()?;
    for part in path_var.split(':') {
        if part.is_empty() {
            continue;
        }
        let candidate = Path::new(part).join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

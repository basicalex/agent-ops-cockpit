use crate::pulse_ipc::{LayoutPane, LayoutTab};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    process::Command,
};

#[derive(Debug, Clone, Default)]
pub struct ZellijQuerySnapshot {
    pub pane_ids: HashSet<String>,
    pub tabs: Vec<LayoutTab>,
    pub panes: Vec<LayoutPane>,
    pub project_tabs: HashMap<String, LayoutTab>,
    pub current_tab_id: Option<String>,
    pub current_tab_index: Option<u64>,
    pub current_tab_hide_floating_panes: Option<bool>,
}

pub fn query_session_snapshot(session_id: &str) -> Result<Option<ZellijQuerySnapshot>, String> {
    if session_id.trim().is_empty() {
        return Ok(Some(ZellijQuerySnapshot::default()));
    }

    let tabs_value = match run_action_json(session_id, &["action", "list-tabs", "--json"])? {
        Some(value) => value,
        None => return Ok(None),
    };
    let panes_value = match run_action_json(session_id, &["action", "list-panes", "--json"])? {
        Some(value) => value,
        None => return Ok(None),
    };
    let current_tab_value = run_action_json(session_id, &["action", "current-tab-info", "--json"])
        .ok()
        .flatten();

    let tabs = parse_tabs(&tabs_value);
    let panes = parse_panes(&panes_value, &tabs);
    let pane_ids = panes.iter().map(|pane| pane.pane_id.clone()).collect();
    let project_tabs = derive_project_tabs(&panes_value, &tabs);
    let (current_tab_id, current_tab_index, current_tab_hide_floating_panes) =
        parse_current_tab(current_tab_value.as_ref());

    Ok(Some(ZellijQuerySnapshot {
        pane_ids,
        tabs,
        panes,
        project_tabs,
        current_tab_id,
        current_tab_index,
        current_tab_hide_floating_panes,
    }))
}

fn run_action_json(session_id: &str, args: &[&str]) -> Result<Option<Value>, String> {
    let output = Command::new("zellij")
        .arg("--session")
        .arg(session_id)
        .args(args)
        .output()
        .map_err(|err| format!("spawn_error:{err}"))?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).map_err(|err| format!("utf8:{err}"))?;
        let trimmed = stdout.trim();
        if trimmed.is_empty() {
            return Ok(Some(Value::Null));
        }
        let value = serde_json::from_str(trimmed).map_err(|err| format!("json:{err}"))?;
        return Ok(Some(value));
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    if stderr.contains("wasn't recognized")
        || stderr.contains("found argument")
        || stderr.contains("isn't valid in this context")
    {
        return Ok(None);
    }

    Err(format!("zellij_exit:{}", output.status))
}

fn parse_tabs(value: &Value) -> Vec<LayoutTab> {
    let items = value_array(value);
    let mut tabs = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let index = value_u64(obj.get("position"))
            .or_else(|| value_u64(obj.get("index")))
            .or_else(|| value_u64(obj.get("tab_index")))
            .or_else(|| value_u64(obj.get("tabPosition")))
            .unwrap_or((idx + 1) as u64);
        let name = value_string(obj.get("name")).unwrap_or_else(|| format!("tab-{index}"));
        let focused = value_bool(obj.get("active"))
            .or_else(|| value_bool(obj.get("is_active")))
            .or_else(|| value_bool(obj.get("focused")))
            .or_else(|| value_bool(obj.get("is_focused")))
            .unwrap_or(false);
        tabs.push(LayoutTab {
            index,
            name,
            focused,
        });
    }
    tabs.sort_by(|left, right| {
        left.index
            .cmp(&right.index)
            .then_with(|| left.name.cmp(&right.name))
    });
    tabs
}

fn parse_panes(value: &Value, tabs: &[LayoutTab]) -> Vec<LayoutPane> {
    let items = value_array(value);
    let tab_by_index: HashMap<u64, LayoutTab> =
        tabs.iter().cloned().map(|tab| (tab.index, tab)).collect();
    let mut tab_name_to_index: HashMap<String, u64> = HashMap::new();
    for tab in tabs {
        tab_name_to_index.insert(tab.name.clone(), tab.index);
    }

    let mut panes = Vec::new();
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let Some(pane_id) = value_string(obj.get("id"))
            .or_else(|| value_string(obj.get("pane_id")))
            .or_else(|| value_string(obj.get("paneId")))
        else {
            continue;
        };

        let mut tab_index = value_u64(obj.get("tab_position"))
            .or_else(|| value_u64(obj.get("tab_index")))
            .or_else(|| value_u64(obj.get("position")));
        let mut tab_name = value_string(obj.get("tab_name"));
        let mut tab_focused = value_bool(obj.get("tab_focused")).unwrap_or(false);

        if tab_index.is_none() {
            if let Some(name) = tab_name.as_ref() {
                tab_index = tab_name_to_index.get(name).copied();
            }
        }
        if let Some(index) = tab_index {
            if let Some(tab) = tab_by_index.get(&index) {
                if tab_name.is_none() {
                    tab_name = Some(tab.name.clone());
                }
                tab_focused = tab.focused;
            }
        }

        let Some(tab_index) = tab_index else {
            continue;
        };
        panes.push(LayoutPane {
            pane_id,
            tab_index,
            tab_name: tab_name.unwrap_or_else(|| format!("tab-{tab_index}")),
            tab_focused,
        });
    }

    panes.sort_by(|left, right| {
        left.tab_index
            .cmp(&right.tab_index)
            .then_with(|| {
                pane_id_number_u64(&left.pane_id)
                    .unwrap_or(u64::MAX)
                    .cmp(&pane_id_number_u64(&right.pane_id).unwrap_or(u64::MAX))
            })
            .then_with(|| left.pane_id.cmp(&right.pane_id))
    });
    panes
}

fn derive_project_tabs(value: &Value, tabs: &[LayoutTab]) -> HashMap<String, LayoutTab> {
    let items = value_array(value);
    let tab_by_index: HashMap<u64, LayoutTab> =
        tabs.iter().cloned().map(|tab| (tab.index, tab)).collect();
    let tab_name_to_index: HashMap<String, u64> = tabs
        .iter()
        .map(|tab| (tab.name.clone(), tab.index))
        .collect();

    let mut project_tabs = HashMap::new();
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let Some(name) = value_string(obj.get("title"))
            .or_else(|| value_string(obj.get("name")))
            .or_else(|| value_string(obj.get("pane_name")))
        else {
            continue;
        };
        if !is_agentish_title(&name) {
            continue;
        }
        let cwd = value_string(obj.get("cwd"))
            .or_else(|| value_string(obj.get("current_working_directory")))
            .or_else(|| value_string(obj.get("current_cwd")))
            .or_else(|| value_string(obj.get("working_dir")))
            .or_else(|| value_string(obj.get("working_directory")))
            .or_else(|| value_string(obj.get("path")));
        let project_root = match cwd {
            Some(cwd) if cwd.starts_with('/') => cwd,
            _ => continue,
        };

        let tab_index = value_u64(obj.get("tab_position"))
            .or_else(|| value_u64(obj.get("tab_index")))
            .or_else(|| value_u64(obj.get("position")))
            .or_else(|| {
                value_string(obj.get("tab_name"))
                    .and_then(|tab_name| tab_name_to_index.get(&tab_name).copied())
            });
        let Some(tab_index) = tab_index else {
            continue;
        };
        let Some(tab) = tab_by_index.get(&tab_index) else {
            continue;
        };

        project_tabs
            .entry(project_root)
            .or_insert_with(|| tab.clone());
    }

    project_tabs
}

fn is_agentish_title(name: &str) -> bool {
    let trimmed = name.trim();
    trimmed == "Agent"
        || trimmed.starts_with("Agent[")
        || trimmed.starts_with("Agent [")
        || trimmed.starts_with("aoc:")
        || trimmed.starts_with("π -")
        || trimmed.starts_with("Pi -")
}

fn parse_current_tab(value: Option<&Value>) -> (Option<String>, Option<u64>, Option<bool>) {
    let Some(value) = value else {
        return (None, None, None);
    };
    let obj = value_object(value);
    let tab_id = obj
        .and_then(|obj| {
            obj.get("tab_id")
                .or_else(|| obj.get("id"))
                .or_else(|| obj.get("tabId"))
        })
        .and_then(|value| value_string(Some(value)));
    let tab_index = obj
        .and_then(|obj| {
            obj.get("position")
                .or_else(|| obj.get("index"))
                .or_else(|| obj.get("tab_index"))
                .or_else(|| obj.get("tabPosition"))
        })
        .and_then(|value| value_u64(Some(value)));
    let hidden = obj.and_then(|obj| {
        value_bool(obj.get("hide_floating_panes")).or_else(|| {
            value_bool(obj.get("floating_panes_visible"))
                .or_else(|| value_bool(obj.get("are_floating_panes_visible")))
                .map(|visible| !visible)
        })
    });
    (tab_id, tab_index, hidden)
}

fn value_array(value: &Value) -> Vec<Value> {
    if let Some(items) = value.as_array() {
        return items.clone();
    }
    if let Some(obj) = value.as_object() {
        for key in ["tabs", "panes", "data", "items"] {
            if let Some(items) = obj.get(key).and_then(Value::as_array) {
                return items.clone();
            }
        }
    }
    Vec::new()
}

fn value_object(value: &Value) -> Option<&serde_json::Map<String, Value>> {
    if let Some(obj) = value.as_object() {
        for key in ["tab", "current_tab", "data"] {
            if let Some(child) = obj.get(key).and_then(Value::as_object) {
                return Some(child);
            }
        }
        return Some(obj);
    }
    None
}

fn value_string(value: Option<&Value>) -> Option<String> {
    let value = value?;
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn value_u64(value: Option<&Value>) -> Option<u64> {
    let value = value?;
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn value_bool(value: Option<&Value>) -> Option<bool> {
    let value = value?;
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::Number(number) => Some(number.as_u64().unwrap_or(0) != 0),
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn pane_id_number_u64(pane_id: &str) -> Option<u64> {
    pane_id.trim().trim_start_matches('%').parse::<u64>().ok()
}

#[allow(dead_code)]
fn resolve_cwd(base_cwd: Option<&str>, cwd: &str) -> Option<String> {
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

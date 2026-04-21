//! Local operations for launch/navigation/evidence flows.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

pub(crate) fn in_zellij_session() -> bool {
    env::var("ZELLIJ").is_ok() || env::var("ZELLIJ_SESSION_NAME").is_ok()
}

pub(crate) fn resolve_launch_agent_id() -> String {
    for key in ["AOC_LAUNCH_AGENT_ID", "AOC_AGENT_ID"] {
        if let Ok(value) = env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    if let Ok(output) = Command::new("aoc-agent").arg("--current").output() {
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !value.is_empty() {
                return value;
            }
        }
    }

    "pi".to_string()
}

pub(crate) fn sanitize_slug(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut last_dash = false;
    for ch in value.chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

pub(crate) fn build_worker_launch_plan(
    project_root: &Path,
    agent_id: &str,
    tab_name: &str,
    brief_path: Option<&Path>,
    in_zellij: bool,
) -> WorkerLaunchPlan {
    let mut env = vec![("AOC_LAUNCH_AGENT_ID".to_string(), agent_id.to_string())];
    if let Some(path) = brief_path {
        env.push((
            "AOC_DELEGATION_BRIEF_PATH".to_string(),
            path.display().to_string(),
        ));
    }

    if in_zellij {
        WorkerLaunchPlan {
            program: "aoc-new-tab".to_string(),
            args: vec![
                "--aoc".to_string(),
                "--name".to_string(),
                tab_name.to_string(),
                "--cwd".to_string(),
                project_root.display().to_string(),
            ],
            env,
            cwd: project_root.to_path_buf(),
            tab_name: tab_name.to_string(),
        }
    } else {
        WorkerLaunchPlan {
            program: "aoc-launch".to_string(),
            args: Vec::new(),
            env,
            cwd: project_root.to_path_buf(),
            tab_name: tab_name.to_string(),
        }
    }
}

pub(crate) fn execute_worker_launch_plan(plan: &WorkerLaunchPlan) -> Result<(), String> {
    let mut cmd = Command::new(&plan.program);
    cmd.current_dir(&plan.cwd);
    for (key, value) in &plan.env {
        cmd.env(key, value);
    }
    cmd.args(&plan.args);
    let status = cmd
        .status()
        .map_err(|err| format!("{} failed: {err}", plan.program))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} exited with {}", plan.program, status))
    }
}

pub(crate) fn dump_pane_evidence(
    session_id: &str,
    pane_id: &str,
    output_path: &Path,
) -> Result<(), String> {
    if pane_id.trim().is_empty() {
        return Err("empty pane id".to_string());
    }
    let output = Command::new("aoc-pane-evidence")
        .arg("--pane-id")
        .arg(pane_id)
        .arg("--session")
        .arg(session_id)
        .output()
        .map_err(|err| format!("aoc-pane-evidence failed: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            return Err(format!("aoc-pane-evidence exited with {}", output.status));
        }
        return Err(stderr);
    }
    fs::write(output_path, &output.stdout)
        .map_err(|err| format!("write evidence failed: {err}"))?;
    Ok(())
}

pub(crate) fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn launch_pane_follow(
    session_id: &str,
    pane_id: &str,
    label: &str,
    project_root: &Path,
) -> Result<(), String> {
    if pane_id.trim().is_empty() {
        return Err("empty pane id".to_string());
    }
    let follow_cmd = format!(
        "exec aoc-pane-evidence --pane-id {} --session {} --follow --scrollback 300",
        shell_single_quote(pane_id),
        shell_single_quote(session_id)
    );
    let title = format!("Follow {}", ellipsize(label, 18));
    let mut cmd = Command::new("zellij");
    cmd.arg("action")
        .arg("new-pane")
        .arg("--floating")
        .arg("--close-on-exit")
        .arg("--borderless")
        .arg("true")
        .arg("--name")
        .arg(&title)
        .arg("--cwd")
        .arg(project_root)
        .arg("--")
        .arg("bash")
        .arg("-lc")
        .arg(follow_cmd);
    let status = cmd
        .status()
        .map_err(|err| format!("zellij new-pane failed: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("zellij action new-pane exited with {}", status))
    }
}

pub(crate) fn go_to_tab(session_id: &str, tab_index: usize) -> Result<(), String> {
    if tab_index == 0 {
        return Err("invalid tab index".to_string());
    }
    let status = Command::new("zellij")
        .arg("--session")
        .arg(session_id)
        .arg("action")
        .arg("go-to-tab")
        .arg(tab_index.to_string())
        .status()
        .map_err(|_| "zellij not available".to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("zellij exited with {}", status))
    }
}

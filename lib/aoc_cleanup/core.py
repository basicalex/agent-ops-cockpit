#!/usr/bin/env python3
"""
AOC Cleanup Core Module

Provides process cleanup logic for identifying and killing orphaned agent processes.
"""

import sys
import subprocess
import os
import signal
import re
import time
import collections
import shlex
import json
import argparse


AGENT_PATTERN = r"opencode|open-code|codex|gemini|claude|kimi|pi-agent|pi_agent|aoc-pi"
TARGET_PATTERN = r"({})".format(AGENT_PATTERN)

PROTECTED_COMMANDS = [
    "aoc-session-watch",
    "aoc-cleanup",
    "aoc-init",
    "aoc-launch",
    "aoc-align",
    "aoc-doctor",
    "aoc-mem",
    "aoc-tasks",
    "aoc-widget",
    "aoc-clock",
    "aoc-sys",
]

EXTRA_AGENT_PATTERN = os.environ.get("AOC_AGENT_PATTERN", "").strip()
if EXTRA_AGENT_PATTERN:
    combined = f"({AGENT_PATTERN}|{EXTRA_AGENT_PATTERN})"
else:
    combined = AGENT_PATTERN

AGENT_NAME_PATTERN = re.compile(f"({combined})", re.IGNORECASE)
AGENT_BINARIES = {
    "opencode",
    "open-code",
    "codex",
    "gemini",
    "claude",
    "kimi",
    "pi",
    "pi-agent",
    "pi_agent",
    "aoc-agent",
    "aoc-agent-run",
    "aoc-oc",
    "aoc-cc",
    "aoc-gemini",
    "aoc-kimi",
    "aoc-pi",
}

ENV_CACHE = {}


def run_cmd(args):
    try:
        return subprocess.check_output(args, text=True, stderr=subprocess.DEVNULL)
    except Exception:
        return ""


def get_current_session():
    return (
        os.environ.get("ZELLIJ_SESSION_NAME") or os.environ.get("ZELLIJ_SESSION") or ""
    )


def list_sessions():
    out = run_cmd(["zellij", "list-sessions", "--short", "--no-formatting"])
    names = []
    if out:
        for line in out.strip().splitlines():
            name = line.strip()
            if name:
                names.append(name)
        if names:
            return names

    out = run_cmd(["zellij", "list-sessions"])
    if not out:
        return names
    ansi_re = re.compile(r"\x1b\[[0-9;]*m")
    for line in out.strip().splitlines():
        cleaned = ansi_re.sub("", line).strip()
        if not cleaned:
            continue
        parts = cleaned.split()
        if parts:
            names.append(parts[0])
    return names


def parse_session_filter():
    raw = os.environ.get("AOC_CLEANUP_SESSIONS", "").strip()
    if not raw:
        return None
    raw_lower = raw.lower()
    if raw_lower in {"current", "self", "this"}:
        curr = get_current_session()
        if curr:
            return {curr}
        print(
            "Warning: AOC_CLEANUP_SESSIONS=current but no current session detected; falling back to all sessions."
        )
        return None
    sessions = {s for s in re.split(r"[\s,]+", raw) if s}
    return sessions


def env_truthy(name):
    raw = os.environ.get(name, "").strip().lower()
    return raw not in {"", "0", "false", "no", "off"}


def env_int(name, default=0, minimum=0):
    raw = os.environ.get(name, "").strip()
    if not raw:
        return default
    try:
        value = int(raw)
    except ValueError:
        print(f"Warning: {name} must be an integer; using {default}.")
        return default
    if value < minimum:
        return minimum
    return value


dry_run = env_truthy("AOC_CLEANUP_DRY_RUN")
require_active_signals = env_truthy("AOC_CLEANUP_REQUIRE_ACTIVE_SIGNALS")
skip_if_no_sessions = env_truthy("AOC_CLEANUP_SKIP_IF_NO_SESSIONS")
min_process_age_secs = env_int("AOC_CLEANUP_MIN_PROCESS_AGE_SECS", 0, 0)


def dump_layout(session=None):
    if session:
        return run_cmd(["zellij", "-s", session, "action", "dump-layout"])
    return run_cmd(["zellij", "action", "dump-layout"])


def detect_projects_base():
    env_base = os.environ.get("AOC_PROJECTS_BASE")
    if env_base:
        return env_base
    config_path = os.environ.get("AOC_CONFIG_PATH")
    if not config_path:
        config_path = os.path.join(
            os.path.expanduser("~"), ".config", "aoc", "config.toml"
        )
    try:
        with open(config_path, "r", encoding="utf-8") as fh:
            for line in fh:
                m = re.match(r'\s*projects_base\s*=\s*"(.*)"\s*', line)
                if m:
                    return m.group(1)
    except Exception:
        return ""

    home = os.path.expanduser("~")
    dev_guess = os.path.join(home, "dev")
    if os.path.isdir(dev_guess):
        return dev_guess
    return home


def normalize_cwd(cwd, projects_base=None):
    if not cwd:
        return ""
    if cwd.startswith("/"):
        return cwd
    if projects_base:
        return os.path.join(projects_base, cwd)
    return cwd


def is_real_agent_command(cmd):
    if not cmd:
        return False
    base = os.path.basename(cmd)
    if base in {
        "aoc-agent",
        "aoc-agent-run",
        "aoc-oc",
        "aoc-cc",
        "aoc-gemini",
        "aoc-kimi",
        "aoc-pi",
        "bash",
    }:
        return False
    return AGENT_NAME_PATTERN.search(cmd) is not None


def is_agent_process(args, comm):
    cmd = proc_cmd(args, comm)
    base = os.path.basename(cmd) if cmd else ""
    if (
        base in AGENT_BINARIES
        or base.startswith("opencode")
        or base.startswith("pi-agent")
    ):
        return True
    if comm == "node" and "opencode" in (args or ""):
        return True
    if AGENT_NAME_PATTERN.search(args or "") or AGENT_NAME_PATTERN.search(comm or ""):
        return True
    return False


def parse_layout(text, projects_base=None):
    active_commands = set()
    agent_cwds = set()
    agent_ids = set()
    expected_cwds = collections.Counter()
    expected_ids = collections.Counter()

    tabs = []
    current_tab = None

    def flush_tab():
        nonlocal current_tab
        if current_tab and current_tab["name"]:
            tabs.append(current_tab)
        current_tab = None

    for line in text.splitlines():
        tab_match = re.search(r'tab\s+name="([^"]+)"', line)
        if tab_match:
            flush_tab()
            current_tab = {
                "name": tab_match.group(1),
                "cwds": [],
                "agent_ids": [],
                "agent_count": 0,
            }
            continue

        if current_tab is None:
            current_tab = {"name": "", "cwds": [], "agent_ids": [], "agent_count": 0}

        if "name=" in line:
            name_match = re.search(r'name\s*=\s*"([^"]+)"', line)
            if not name_match:
                name_match = re.search(r'name\s+"([^"]+)"', line)
            if name_match:
                name = name_match.group(1).strip()
                if name.startswith("Agent [") and "]" in name:
                    agent_id = name.split("[", 1)[1].split("]", 1)[0].strip()
                    if agent_id:
                        agent_ids.add(agent_id)
                        expected_ids[agent_id] += 1
                        current_tab["agent_ids"].append(agent_id)
                        current_tab["agent_count"] += 1
                if name.startswith("aoc:"):
                    agent_id = name.split(":", 1)[1].strip()
                    if agent_id:
                        agent_ids.add(agent_id)
                        expected_ids[agent_id] += 1

        if "command" in line or "args" in line or "cwd" in line:
            quoted = re.findall(r'"([^"]+)"', line)
            for token in quoted:
                if AGENT_NAME_PATTERN.search(token):
                    active_commands.add(token)
                cd_match = re.search(r'\bcd\s+"([^"]+)"', token)
                if cd_match:
                    agent_cwds.add(normalize_cwd(cd_match.group(1), projects_base))

            cwd_match = re.search(r'cwd\s*=\s*"([^"]+)"', line)
            if not cwd_match:
                cwd_match = re.search(r'cwd\s+"([^"]+)"', line)
            if cwd_match:
                cwd = normalize_cwd(cwd_match.group(1), projects_base)
                if cwd:
                    agent_cwds.add(cwd)
                    current_tab["cwds"].append(cwd)

            cmd_match = re.search(r'command\s*=\s*"([^"]+)"', line)
            if not cmd_match:
                cmd_match = re.search(r'command\s+"([^"]+)"', line)
            if cmd_match:
                cmd = cmd_match.group(1)
                if AGENT_NAME_PATTERN.search(cmd):
                    active_commands.add(cmd)

    flush_tab()

    for tab in tabs:
        if not tab["cwds"]:
            continue
        counts = collections.Counter(tab["cwds"])
        root = counts.most_common(1)[0][0]
        if tab["agent_count"] > 0:
            expected_cwds[root] += tab["agent_count"]

    if projects_base:
        for agent_id, count in expected_ids.items():
            guess = os.path.join(projects_base, agent_id)
            if os.path.isdir(guess):
                agent_cwds.add(guess)
                if guess not in expected_cwds:
                    expected_cwds[guess] += count

    return active_commands, agent_cwds, expected_cwds, expected_ids


def parse_layout_pane_ids(text):
    pane_ids = set()
    for match in re.finditer(r'--pane-id"\s*"([^"]+)"', text):
        pane_id = (match.group(1) or "").strip()
        if pane_id:
            pane_ids.add(pane_id)
    if pane_ids:
        return pane_ids
    for part in text.split('--pane-id"')[1:]:
        quote = part.find('"')
        if quote == -1:
            continue
        tail = part[quote + 1 :]
        end = tail.find('"')
        if end == -1:
            continue
        pane_id = tail[:end].strip()
        if pane_id:
            pane_ids.add(pane_id)
    return pane_ids


def build_active_pane_map(allowed_sessions=None):
    pane_map = {}
    sessions = list_sessions()
    if allowed_sessions is not None:
        sessions = [s for s in sessions if s in allowed_sessions]
    if sessions:
        for session in sessions:
            layout = dump_layout(session)
            if not layout:
                continue
            pane_ids = parse_layout_pane_ids(layout)
            if pane_ids:
                pane_map[session] = pane_ids
        return pane_map

    if allowed_sessions is None:
        curr = get_current_session()
        layout = dump_layout()
        if curr and layout:
            pane_ids = parse_layout_pane_ids(layout)
            if pane_ids:
                pane_map[curr] = pane_ids
    else:
        curr = get_current_session()
        if curr and curr in allowed_sessions:
            layout = dump_layout()
            if layout:
                pane_ids = parse_layout_pane_ids(layout)
                if pane_ids:
                    pane_map[curr] = pane_ids
    return pane_map


def build_active_sets(allowed_sessions=None):
    active_commands = set()
    agent_cwds = set()
    expected_cwds = collections.Counter()
    expected_ids = collections.Counter()
    projects_base = detect_projects_base()
    sessions = list_sessions()
    if allowed_sessions is not None:
        sessions = [s for s in sessions if s in allowed_sessions]
    if sessions:
        for session in sessions:
            layout = dump_layout(session)
            if not layout:
                continue
            cmds, cwds, exp, ids = parse_layout(layout, projects_base)
            active_commands.update(cmds)
            agent_cwds.update(cwds)
            expected_cwds.update(exp)
            expected_ids.update(ids)
        return active_commands, agent_cwds, expected_cwds, expected_ids

    if allowed_sessions is None:
        layout = dump_layout()
        if layout:
            cmds, cwds, exp, ids = parse_layout(layout, projects_base)
            active_commands.update(cmds)
            agent_cwds.update(cwds)
            expected_cwds.update(exp)
            expected_ids.update(ids)
    else:
        curr = get_current_session()
        if curr and curr in allowed_sessions:
            layout = dump_layout()
            if layout:
                cmds, cwds, exp, ids = parse_layout(layout, projects_base)
                active_commands.update(cmds)
                agent_cwds.update(cwds)
                expected_cwds.update(exp)
                expected_ids.update(ids)
    return active_commands, agent_cwds, expected_cwds, expected_ids


def proc_env(pid):
    if pid in ENV_CACHE:
        return ENV_CACHE[pid]
    try:
        with open(f"/proc/{pid}/environ", "rb") as fh:
            data = fh.read().split(b"\0")
    except Exception:
        ENV_CACHE[pid] = {}
        return ENV_CACHE[pid]
    env = {}
    for item in data:
        if not item or b"=" not in item:
            continue
        key, value = item.split(b"=", 1)
        try:
            k = key.decode("utf-8", errors="ignore")
            v = value.decode("utf-8", errors="ignore")
        except Exception:
            continue
        if k:
            env[k] = v
    ENV_CACHE[pid] = env
    return env


def proc_has_active_session(pid, active_sessions):
    if not active_sessions:
        return False
    env = proc_env(pid)
    session = (
        env.get("AOC_SESSION_ID")
        or env.get("ZELLIJ_SESSION_NAME")
        or env.get("ZELLIJ_SESSION")
    )
    if not session:
        return False
    return session in active_sessions


def proc_matches_active_pane(proc, active_panes_by_session):
    session = (proc.get("session") or "").strip()
    pane_id = (proc.get("pane_id") or "").strip()
    if not session or not pane_id:
        return False
    active = active_panes_by_session.get(session)
    if not active:
        return False
    return pane_id in active


def telemetry_root_dir():
    state_home = os.environ.get("XDG_STATE_HOME", "").strip()
    if state_home:
        base = state_home
    else:
        base = os.path.join(os.path.expanduser("~"), ".local", "state")
    return os.path.join(base, "aoc", "telemetry")


def prune_runtime_snapshots(
    active_panes_by_session, active_sessions, session_filter_active, dry_run=False
):
    root = telemetry_root_dir()
    if not os.path.isdir(root):
        return 0
    removed = 0
    for session_name in os.listdir(root):
        session_path = os.path.join(root, session_name)
        if not os.path.isdir(session_path):
            continue
        if (
            session_filter_active
            and active_sessions
            and session_name not in active_sessions
        ):
            continue
        allowed_panes = active_panes_by_session.get(session_name, set())
        for file_name in os.listdir(session_path):
            if not file_name.endswith(".json"):
                continue
            pane_id = file_name[:-5].strip()
            if pane_id and pane_id in allowed_panes:
                continue
            path = os.path.join(session_path, file_name)
            keep = False
            try:
                with open(path, "r", encoding="utf-8") as fh:
                    snapshot = json.load(fh)
                pid = int(snapshot.get("pid") or 0)
                if pid > 0:
                    env = proc_env(pid)
                    proc_session = (
                        env.get("AOC_SESSION_ID")
                        or env.get("ZELLIJ_SESSION_NAME")
                        or env.get("ZELLIJ_SESSION")
                        or ""
                    )
                    proc_pane = (
                        env.get("AOC_PANE_ID") or env.get("ZELLIJ_PANE_ID") or ""
                    )
                    if proc_session == session_name and proc_pane == pane_id:
                        keep = True
            except Exception:
                pass
            if keep:
                continue
            try:
                if dry_run:
                    removed += 1
                else:
                    os.remove(path)
                    removed += 1
            except FileNotFoundError:
                pass
            except Exception:
                pass
    return removed


def proc_cwd(pid):
    try:
        return os.readlink(f"/proc/{pid}/cwd")
    except Exception:
        return ""


def cwd_matches(proc_path, active_cwd):
    if not proc_path or not active_cwd:
        return False
    if active_cwd.startswith("/"):
        return proc_path == active_cwd or proc_path.startswith(active_cwd + "/")
    return proc_path == active_cwd or proc_path.endswith("/" + active_cwd)


def cmd_matches(proc_cmd, active_cmd):
    if not proc_cmd or not active_cmd:
        return False
    if proc_cmd == active_cmd:
        return True
    try:
        return os.path.basename(proc_cmd) == os.path.basename(active_cmd)
    except Exception:
        return False


def proc_cmd(args, comm):
    if args:
        try:
            parts = shlex.split(args)
            if parts:
                return parts[0]
        except Exception:
            pass
        return args.split(" ", 1)[0]
    return comm


def get_process_map():
    try:
        ps_out = subprocess.check_output(
            ["ps", "-eo", "pid,ppid,etimes,tty,comm,args"], text=True
        )
    except subprocess.CalledProcessError:
        return {}, {}

    processes = {}
    children = collections.defaultdict(list)

    for line in ps_out.strip().splitlines()[1:]:
        parts = line.strip().split(None, 5)
        if len(parts) < 6:
            continue
        try:
            pid = int(parts[0])
            ppid = int(parts[1])
            etimes_raw = parts[2]
            tty = parts[3]
            comm = parts[4]
            args = parts[5]
            try:
                etimes = int(etimes_raw)
            except ValueError:
                etimes = None
            processes[pid] = {
                "ppid": ppid,
                "etimes": etimes,
                "tty": tty,
                "comm": comm,
                "args": args,
            }
            children[ppid].append(pid)
        except ValueError:
            continue

    return processes, children


def get_descendants(roots, children_map):
    safe = set(roots)
    stack = list(roots)
    while stack:
        curr = stack.pop()
        for child in children_map.get(curr, []):
            if child not in safe:
                safe.add(child)
                stack.append(child)
    return safe


def find_safe_tmux_sockets(safe_pids, processes):
    sockets = set()
    socket_re = re.compile(r"tmux\s+.*-(L|S)\s*([a-zA-Z0-9_\-\./]+)")

    for pid in safe_pids:
        if pid not in processes:
            continue
        args = processes[pid]["args"]
        if "tmux" in args:
            match = socket_re.search(args)
            if match:
                s = match.group(2)
                sockets.add(s)
    return sockets


def find_tmux_servers_for_sockets(sockets, processes):
    server_pids = set()
    if not sockets:
        return server_pids
    socket_re = re.compile(r"tmux\s+.*-(L|S)\s*([a-zA-Z0-9_\-\./]+)")
    for pid, info in processes.items():
        if info["comm"] == "tmux" and " -D" in info["args"]:
            match = socket_re.search(info["args"])
            if match and match.group(2) in sockets:
                server_pids.add(pid)
    return server_pids


def run_cleanup():
    global dry_run, require_active_signals, skip_if_no_sessions, min_process_age_secs

    processes, children = get_process_map()

    session_filter = parse_session_filter()
    pane_strict = env_truthy("AOC_CLEANUP_PANE_STRICT")
    all_sessions = list_sessions()
    if skip_if_no_sessions and not all_sessions:
        print(
            "No active Zellij sessions found; skipping cleanup (AOC_CLEANUP_SKIP_IF_NO_SESSIONS=1)."
        )
        return
    if session_filter is None:
        active_sessions = set(all_sessions)
    else:
        if session_filter:
            if all_sessions:
                active_sessions = {s for s in all_sessions if s in session_filter}
                if not active_sessions:
                    active_sessions = set(session_filter)
            else:
                active_sessions = set(session_filter)
        else:
            active_sessions = set()

    active_commands, agent_cwds, expected_cwds, expected_ids = build_active_sets(
        active_sessions if session_filter is not None else None
    )
    active_panes_by_session = build_active_pane_map(
        active_sessions if session_filter is not None else None
    )
    have_active_signals = bool(
        active_commands or agent_cwds or expected_cwds or expected_ids
    )
    pane_layout_signals = bool(expected_cwds or expected_ids or active_panes_by_session)

    zellij_pids = set()
    for pid, info in processes.items():
        if info["comm"] == "zellij":
            zellij_pids.add(pid)

    if not zellij_pids:
        print("Warning: No active Zellij sessions found. All agents will be orphans.")

    safe_pids = get_descendants(zellij_pids, children)

    my_pid = os.getpid()
    curr = my_pid
    while curr in processes:
        safe_pids.add(curr)
        curr = processes[curr]["ppid"]

    safe_sockets = find_safe_tmux_sockets(safe_pids, processes)
    if safe_sockets:
        print(f"Found active tmux sockets in safe tree: {safe_sockets}")
        tmux_server_roots = find_tmux_servers_for_sockets(safe_sockets, processes)
        safe_pids.update(get_descendants(tmux_server_roots, children))

    if active_commands:
        print(f"Active agent commands in panes: {len(active_commands)}")
    if agent_cwds:
        print(f"Active agent CWD hints: {len(agent_cwds)}")
    if expected_cwds:
        print(f"Active agent panes detected: {sum(expected_cwds.values())}")
    if expected_ids:
        print(f"Active agent ids detected: {sum(expected_ids.values())}")
    if active_panes_by_session:
        total_active_panes = sum(
            len(panes) for panes in active_panes_by_session.values()
        )
        print(f"Active pane ids discovered: {total_active_panes}")
    if session_filter is not None:
        if active_sessions:
            print(f"Session filter active: {', '.join(sorted(active_sessions))}")
        else:
            print("Warning: Session filter active but no matching sessions found.")
    if pane_strict:
        print("Pane strict mode: on")
    if dry_run:
        print("Dry run mode: on")
    if not have_active_signals:
        print(
            "Warning: No active agent panes discovered; falling back to safe-pid logic only."
        )
    if require_active_signals and not have_active_signals:
        print(
            "Skipping cleanup because active pane signals are required (AOC_CLEANUP_REQUIRE_ACTIVE_SIGNALS=1)."
        )
        return
    if min_process_age_secs > 0:
        print(f"Minimum process age filter: {min_process_age_secs}s")

    kill_list = []
    agent_candidates = []

    for pid, info in processes.items():
        comm = info["comm"]
        args = info["args"]

        if is_agent_process(args, comm):
            if any(p in comm or p in args for p in PROTECTED_COMMANDS):
                continue

            if any(x in args for x in [" grep ", " pgrep ", " ps "]):
                continue

            cwd = proc_cwd(pid)
            cmd = proc_cmd(args, comm)
            base = os.path.basename(cmd) if cmd else ""
            kind = "other"
            if (
                base in AGENT_BINARIES
                or base.startswith("opencode")
                or base.startswith("pi-agent")
            ):
                kind = "agent_bin"
            elif comm == "node" and "opencode" in (args or ""):
                kind = "agent_node"
            env = proc_env(pid)
            session_name = (
                env.get("AOC_SESSION_ID")
                or env.get("ZELLIJ_SESSION_NAME")
                or env.get("ZELLIJ_SESSION")
                or ""
            )
            pane_id = env.get("AOC_PANE_ID") or env.get("ZELLIJ_PANE_ID") or ""
            pane_instance_id = env.get("AOC_PANE_INSTANCE_ID") or ""
            pane_lease_file = env.get("AOC_PANE_LEASE_FILE") or ""
            pane_instance_match = True
            if pane_instance_id and pane_lease_file:
                try:
                    with open(pane_lease_file, "r", encoding="utf-8") as fh:
                        lease_owner = fh.readline().strip()
                    pane_instance_match = lease_owner == pane_instance_id
                except Exception:
                    pane_instance_match = False
            tty = info.get("tty", "")
            has_tty = bool(tty and tty != "?")
            agent_candidates.append(
                {
                    "pid": pid,
                    "ppid": info.get("ppid"),
                    "etimes": info.get("etimes"),
                    "args": args,
                    "comm": comm,
                    "cwd": cwd,
                    "safe": pid in safe_pids,
                    "kind": kind,
                    "session": session_name,
                    "pane_id": pane_id,
                    "pane_instance_id": pane_instance_id,
                    "pane_lease_file": pane_lease_file,
                    "pane_instance_match": pane_instance_match,
                    "tty": tty,
                    "has_tty": has_tty,
                }
            )

    keep_pids = set()
    used_pane_selection = False

    def root_rank(proc):
        tty_rank = 0 if proc.get("has_tty") else 1
        kind = proc.get("kind")
        if kind == "agent_node":
            kind_rank = 0
        elif kind == "agent_bin":
            kind_rank = 1
        else:
            kind_rank = 2
        return (tty_rank, kind_rank, -proc.get("pid", 0))

    if pane_strict and pane_layout_signals:
        pane_groups = collections.defaultdict(list)
        for proc in agent_candidates:
            pane_id = proc.get("pane_id")
            if not pane_id:
                continue
            proc_session = proc.get("session") or ""
            if active_sessions and proc_session and proc_session not in active_sessions:
                continue
            pane_groups[(proc_session, pane_id)].append(proc)

        if pane_groups:
            used_pane_selection = True
            selected_roots = []
            for (_proc_session, _pane_id), procs in pane_groups.items():
                group_has_tty = any(p.get("has_tty") for p in procs)
                group_has_session = any(p.get("session") for p in procs)
                preferred_pool = [p for p in procs if p.get("has_tty")]
                if not preferred_pool:
                    group_pids = {p["pid"] for p in procs}
                    roots = [p for p in procs if p.get("ppid") not in group_pids]
                    if not roots:
                        roots = procs
                    preferred_pool = roots
                preferred_pool.sort(key=root_rank)
                selected_roots.append(
                    {
                        "proc": preferred_pool[0],
                        "has_tty": group_has_tty,
                        "has_session": group_has_session,
                    }
                )

            expected_total = 0
            if expected_cwds:
                expected_total = sum(expected_cwds.values())
            elif expected_ids:
                expected_total = sum(expected_ids.values())
            if expected_total and len(selected_roots) > expected_total:
                selected_roots.sort(
                    key=lambda r: (
                        not r.get("has_tty"),
                        not r.get("has_session"),
                        root_rank(r["proc"]),
                    )
                )
                selected_roots = selected_roots[:expected_total]
            for entry in selected_roots:
                keep_pids.add(entry["proc"]["pid"])

    expected_cwds_effective = collections.Counter()
    if expected_cwds and not used_pane_selection:
        key_matches = {key: [] for key in expected_cwds.keys()}
        for proc in agent_candidates:
            for cwd_hint, count in expected_cwds.items():
                if not cwd_matches(proc["cwd"], cwd_hint):
                    continue
                key_matches[cwd_hint].append(proc)

        for cwd_hint, procs in key_matches.items():
            if not procs:
                continue
            expected_cwds_effective[cwd_hint] = expected_cwds.get(cwd_hint, 0)
            procs.sort(key=lambda p: p["pid"], reverse=True)
            preferred = [p for p in procs if p["kind"] == "agent_bin"]
            if not preferred:
                preferred = [p for p in procs if p["kind"] == "agent_node"]
            if not preferred:
                preferred = procs
            keep_count = expected_cwds.get(cwd_hint, 0)
            for proc in preferred[:keep_count]:
                keep_pids.add(proc["pid"])
    elif expected_ids and not used_pane_selection:
        key_matches = {key: [] for key in expected_ids.keys()}
        for proc in agent_candidates:
            cwd_base = os.path.basename(proc["cwd"]) if proc["cwd"] else ""
            if not cwd_base:
                continue
            if cwd_base in key_matches:
                key_matches[cwd_base].append(proc)

        for agent_id, procs in key_matches.items():
            if not procs:
                continue
            procs.sort(key=lambda p: p["pid"], reverse=True)
            preferred = [p for p in procs if p["kind"] == "agent_bin"]
            if not preferred:
                preferred = [p for p in procs if p["kind"] == "agent_node"]
            if not preferred:
                preferred = procs
            keep_count = expected_ids.get(agent_id, 0)
            for proc in preferred[:keep_count]:
                keep_pids.add(proc["pid"])

    keep_tree = set()
    if keep_pids:
        keep_tree = get_descendants(keep_pids, children)

    skipped_young = 0
    skipped_pane_miss = 0

    for proc in agent_candidates:
        pid = proc["pid"]
        args = proc["args"]
        proc_session = proc.get("session") or ""
        if (
            session_filter is not None
            and proc_session
            and proc_session not in active_sessions
        ):
            continue
        if proc.get("pane_instance_id") and not proc.get("pane_instance_match", True):
            kill_list.append((pid, args))
            continue
        if proc_matches_active_pane(proc, active_panes_by_session):
            continue
        if (
            proc_session
            and proc.get("pane_id")
            and proc_session in active_panes_by_session
        ):
            if pane_strict and pane_layout_signals:
                kill_list.append((pid, args))
            else:
                if min_process_age_secs <= 0:
                    skipped_pane_miss += 1
                    continue
                proc_age = proc.get("etimes")
                if proc_age is None or proc_age < min_process_age_secs:
                    skipped_pane_miss += 1
                    continue
                kill_list.append((pid, args))
            continue
        if pid in safe_pids:
            if not (pane_strict and pane_layout_signals):
                if session_filter is None or proc_has_active_session(
                    pid, active_sessions
                ):
                    continue
        if pid in keep_tree:
            continue
        if proc_has_active_session(pid, active_sessions):
            if not (pane_strict and pane_layout_signals):
                continue

        active_hit = False
        if have_active_signals and not (pane_strict and pane_layout_signals):
            proc_exec = proc_cmd(proc["args"], proc["comm"])
            if any(
                cmd_matches(proc_exec, cmd) or (cmd in proc["args"])
                for cmd in active_commands
            ):
                active_hit = True
            elif not (proc.get("session") and proc.get("pane_id")):
                if any(cwd_matches(proc["cwd"], ac) for ac in agent_cwds):
                    active_hit = True
        if active_hit:
            continue

        if min_process_age_secs > 0:
            proc_age = proc.get("etimes")
            if proc_age is None or proc_age < min_process_age_secs:
                skipped_young += 1
                continue

        kill_list.append((pid, args))

    if not kill_list:
        print("No orphan processes found.")
    else:
        print(f"Found {len(kill_list)} orphan processes.")
        for pid, args in kill_list:
            display_cmd = (args[:60] + "...") if len(args) > 60 else args
            if dry_run:
                print(f"Would kill PID {pid}: {display_cmd}")
            else:
                print(f"Killing PID {pid}: {display_cmd}")
                try:
                    os.kill(pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass

        if not dry_run:
            time.sleep(0.5)

            for pid, _ in kill_list:
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass

    if skipped_young:
        print(
            f"Skipped {skipped_young} young/unknown-age agent processes (< {min_process_age_secs}s)."
        )
    if skipped_pane_miss:
        print(
            f"Skipped {skipped_pane_miss} pane-miss candidates "
            "(non-strict mode; set AOC_CLEANUP_MIN_PROCESS_AGE_SECS>0 to age out)."
        )

    removed_snapshots = prune_runtime_snapshots(
        active_panes_by_session,
        active_sessions,
        session_filter is not None,
        dry_run,
    )
    if removed_snapshots:
        if dry_run:
            print(f"Would prune {removed_snapshots} stale runtime snapshots.")
        else:
            print(f"Pruned {removed_snapshots} stale runtime snapshots.")

    print("Cleanup complete.")


def main():
    parser = argparse.ArgumentParser(
        description="AOC Cleanup - Identify and kill orphaned agent processes"
    )
    parser.add_argument(
        "-n",
        "--dry-run",
        action="store_true",
        default=None,
        help="Preview actions without killing or deleting files",
    )
    parser.add_argument(
        "--execute",
        action="store_true",
        help="Force execute mode (overrides AOC_CLEANUP_DRY_RUN)",
    )
    args = parser.parse_args()

    if args.dry_run is not None:
        os.environ["AOC_CLEANUP_DRY_RUN"] = "1" if args.dry_run else "0"
    elif args.execute:
        os.environ["AOC_CLEANUP_DRY_RUN"] = "0"

    run_cleanup()


if __name__ == "__main__":
    main()

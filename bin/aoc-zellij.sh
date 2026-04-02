#!/usr/bin/env bash

# Shared Zellij capability/query helpers for AOC.
# Prefer Zellij 0.44+ native JSON inventory when available, but fail open to
# dump-layout parsing on older versions or when python3/json parsing is absent.

_aoc_zellij_supports_action() {
  local action="$1"
  if ! command -v zellij >/dev/null 2>&1; then
    return 1
  fi

  local cache_var="_AOC_ZELLIJ_SUPPORTS_${action//-/_}"
  local cached="${!cache_var-}"
  if [[ -n "$cached" ]]; then
    [[ "$cached" == "1" ]]
    return
  fi

  if zellij action "$action" --help >/dev/null 2>&1; then
    printf -v "$cache_var" '%s' "1"
    return 0
  fi

  printf -v "$cache_var" '%s' "0"
  return 1
}

_aoc_zellij_current_tab_info_json() {
  _aoc_zellij_supports_action current-tab-info || return 1
  command -v python3 >/dev/null 2>&1 || return 1
  zellij action current-tab-info --json 2>/dev/null
}

_aoc_zellij_list_panes_json() {
  _aoc_zellij_supports_action list-panes || return 1
  command -v python3 >/dev/null 2>&1 || return 1
  zellij action list-panes --json 2>/dev/null
}

_aoc_zellij_list_tabs_json() {
  _aoc_zellij_supports_action list-tabs || return 1
  command -v python3 >/dev/null 2>&1 || return 1
  zellij action list-tabs --json 2>/dev/null
}

aoc_zellij_supports_native_inventory() {
  _aoc_zellij_supports_action list-panes \
    && _aoc_zellij_supports_action list-tabs \
    && _aoc_zellij_supports_action current-tab-info
}

aoc_zellij_current_tab_id() {
  local json
  json="$(_aoc_zellij_current_tab_info_json)" || return 1
  AOC_ZELLIJ_JSON="$json" python3 - <<'PY'
import json, os

def data_root(raw):
    val = json.loads(raw)
    if isinstance(val, dict):
        for key in ("tab", "data", "current_tab"):
            child = val.get(key)
            if isinstance(child, dict):
                return child
    return val if isinstance(val, dict) else {}

def first(*values):
    for value in values:
        if value not in (None, ""):
            return value
    return None

obj = data_root(os.environ["AOC_ZELLIJ_JSON"])
value = first(obj.get("tab_id"), obj.get("id"), obj.get("tabId"))
if value not in (None, ""):
    print(value)
    raise SystemExit(0)
raise SystemExit(1)
PY
}

aoc_zellij_current_tab_floating_hidden() {
  local json
  if json="$(_aoc_zellij_current_tab_info_json)"; then
    AOC_ZELLIJ_JSON="$json" python3 - <<'PY'
import json, os

def data_root(raw):
    val = json.loads(raw)
    if isinstance(val, dict):
        for key in ("tab", "data", "current_tab"):
            child = val.get(key)
            if isinstance(child, dict):
                return child
    return val if isinstance(val, dict) else {}

def as_bool(value):
    if isinstance(value, bool):
        return value
    if isinstance(value, (int, float)):
        return bool(value)
    if isinstance(value, str):
        lowered = value.strip().lower()
        if lowered in {"1", "true", "yes", "on"}:
            return True
        if lowered in {"0", "false", "no", "off"}:
            return False
    return None

obj = data_root(os.environ["AOC_ZELLIJ_JSON"])
hidden = as_bool(obj.get("hide_floating_panes"))
if hidden is None:
    visible = as_bool(obj.get("floating_panes_visible"))
    if visible is None:
        visible = as_bool(obj.get("are_floating_panes_visible"))
    if visible is not None:
        hidden = not visible
if hidden is None:
    hidden = False
print("1" if hidden else "0")
PY
    return 0
  fi

  local layout="${1:-}"
  if [[ -z "$layout" ]]; then
    layout="$(zellij action dump-layout 2>/dev/null || true)"
  fi
  if [[ -z "$layout" ]]; then
    echo "0"
    return 0
  fi
  if command -v awk >/dev/null 2>&1; then
    printf '%s\n' "$layout" | awk '
      /tab .*focus=true/ {
        if (index($0, "hide_floating_panes=true") > 0) {
          print 1
        } else {
          print 0
        }
        exit
      }
      END {
        if (NR == 0) {
          print 0
        }
      }
    '
    return 0
  fi
  echo "0"
}

aoc_zellij_show_current_tab_floating() {
  local tab_id
  if tab_id="$(aoc_zellij_current_tab_id 2>/dev/null)" \
    && [[ -n "$tab_id" ]] \
    && _aoc_zellij_supports_action show-floating-panes; then
    zellij action show-floating-panes --tab-id "$tab_id" >/dev/null 2>&1
    return $?
  fi
  zellij action toggle-floating-panes >/dev/null 2>&1
}

aoc_zellij_hide_current_tab_floating() {
  local tab_id
  if tab_id="$(aoc_zellij_current_tab_id 2>/dev/null)" \
    && [[ -n "$tab_id" ]] \
    && _aoc_zellij_supports_action hide-floating-panes; then
    zellij action hide-floating-panes --tab-id "$tab_id" >/dev/null 2>&1
    return $?
  fi
  zellij action toggle-floating-panes >/dev/null 2>&1
}

aoc_zellij_pane_exists() {
  local target_name="$1"
  shift
  local json
  if json="$(_aoc_zellij_list_panes_json)"; then
    AOC_ZELLIJ_JSON="$json" python3 - "$target_name" "$@" <<'PY'
import json, os, re, sys

target = sys.argv[1]
patterns = [re.compile(p) for p in sys.argv[2:]]
val = json.loads(os.environ["AOC_ZELLIJ_JSON"])
items = []
if isinstance(val, list):
    items = val
elif isinstance(val, dict):
    for key in ("panes", "data", "items"):
        child = val.get(key)
        if isinstance(child, list):
            items = child
            break

def first_str(obj, *keys):
    for key in keys:
        value = obj.get(key)
        if value is None:
            continue
        if isinstance(value, str):
            if value:
                return value
        elif isinstance(value, (int, float)):
            return str(value)
    return ""

for pane in items:
    if not isinstance(pane, dict):
        continue
    name = first_str(pane, "name", "pane_name")
    command = first_str(pane, "command", "pane_command", "cmd", "title")
    if name == target or any(p.search(command) for p in patterns):
        raise SystemExit(0)
raise SystemExit(1)
PY
    return $?
  fi

  local layout="$(zellij action dump-layout 2>/dev/null || true)"
  if [[ -z "$layout" ]]; then
    return 1
  fi
  if printf '%s' "$layout" | grep -Eq "name=\"$target_name\""; then
    return 0
  fi
  local pattern
  for pattern in "$@"; do
    if printf '%s' "$layout" | grep -Eq "$pattern"; then
      return 0
    fi
  done
  return 1
}

aoc_zellij_project_root_from_current_tab() {
  local _projects_base="${1:-}"
  if aoc_zellij_current_tab_project_root 2>/dev/null; then
    return 0
  fi
  return 1
}

aoc_zellij_current_tab_project_root() {
  local panes_json current_tab_json
  panes_json="$(_aoc_zellij_list_panes_json)" || return 1
  current_tab_json="$(_aoc_zellij_current_tab_info_json)" || return 1
  AOC_ZELLIJ_PANES_JSON="$panes_json" \
  AOC_ZELLIJ_CURRENT_TAB_JSON="$current_tab_json" \
  python3 - <<'PY'
import json, os, re
panes_raw = os.environ["AOC_ZELLIJ_PANES_JSON"]
current_raw = os.environ["AOC_ZELLIJ_CURRENT_TAB_JSON"]
AGENT_TOKEN_RE = re.compile(r"\b(pi|aoc-pi|aoc-agent-run|aoc-agent-wrap|codex|claude|gemini|opencode|open-code|kimi)\b", re.IGNORECASE)
HELPER_TITLES = {"aoc-refresh-layout-state"}


def load(raw):
    val = json.loads(raw)
    if isinstance(val, list):
        return val
    if isinstance(val, dict):
        for key in ("panes", "data", "items"):
            child = val.get(key)
            if isinstance(child, list):
                return child
    return []


def load_obj(raw):
    val = json.loads(raw)
    if isinstance(val, dict):
        for key in ("tab", "data", "current_tab"):
            child = val.get(key)
            if isinstance(child, dict):
                return child
        return val
    return {}


def first(obj, *keys):
    for key in keys:
        value = obj.get(key)
        if value not in (None, ""):
            return value
    return None


def is_truthy(value):
    if value in (True, 1):
        return True
    if isinstance(value, str):
        return value.strip().lower() in {"1", "true", "yes", "on"}
    return False


def collect_tokens(value):
    if isinstance(value, str):
        stripped = value.strip()
        return [stripped] if stripped else []
    if isinstance(value, list):
        tokens = []
        for item in value:
            tokens.extend(collect_tokens(item))
        return tokens
    if isinstance(value, dict):
        tokens = []
        for item in value.values():
            tokens.extend(collect_tokens(item))
        return tokens
    return []


def pane_title(pane):
    title = first(pane, "title", "name", "pane_name", "pane_title")
    return title if isinstance(title, str) else ""


def is_agentish_title(title):
    stripped = title.strip()
    return (
        stripped == "Agent"
        or stripped.startswith("Agent[")
        or stripped.startswith("Agent [")
        or stripped.startswith("aoc:")
        or stripped.startswith("π -")
        or stripped.startswith("Pi -")
    )


def normalize_root(path):
    real = os.path.realpath(path)
    probe = real
    while probe and probe != "/":
        if os.path.isdir(os.path.join(probe, ".aoc")) or os.path.isdir(os.path.join(probe, ".git")):
            return probe
        parent = os.path.dirname(probe)
        if parent == probe:
            break
        probe = parent
    return real


current = load_obj(current_raw)
current_tab_id = first(current, "tab_id", "id", "tabId")
current_index = first(current, "position", "index", "tab_index", "tabPosition")

candidates = []
for pane in load(panes_raw):
    if not isinstance(pane, dict):
        continue
    tab_id = first(pane, "tab_id", "tabId")
    tab_index = first(pane, "tab_position", "tab_index", "position")
    if current_tab_id is not None and tab_id is not None and str(tab_id) != str(current_tab_id):
        continue
    if current_tab_id is None and current_index is not None and tab_index is not None and str(tab_index) != str(current_index):
        continue
    cwd = first(
        pane,
        "cwd",
        "current_working_directory",
        "current_cwd",
        "working_dir",
        "working_directory",
        "path",
    )
    if not isinstance(cwd, str) or not cwd.startswith("/"):
        continue

    title = pane_title(pane)
    if title in HELPER_TITLES:
        continue

    score = 0
    if is_agentish_title(title):
        score += 60
    if is_truthy(first(pane, "is_focused", "focused", "active")):
        score += 20
    if is_truthy(first(pane, "is_plugin", "plugin")):
        score -= 50

    pane_tokens = []
    for key in ("command", "pane_command", "cmd", "executable", "argv", "args", "command_line"):
        pane_tokens.extend(collect_tokens(pane.get(key)))
    if any(AGENT_TOKEN_RE.search(token) for token in pane_tokens):
        score += 35

    candidates.append((score, len(candidates), normalize_root(cwd)))

if not candidates:
    raise SystemExit(1)

candidates.sort(key=lambda item: (-item[0], item[1]))
print(candidates[0][2])
PY
}

aoc_zellij_current_tab_agent_project_root() {
  aoc_zellij_current_tab_project_root "$@"
}

aoc_zellij_find_current_tab_pane_id_by_name() {
  local target_name="$1"
  [[ -n "$target_name" ]] || return 2
  local panes_json current_tab_json
  panes_json="$(_aoc_zellij_list_panes_json)" || return 1
  current_tab_json="$(_aoc_zellij_current_tab_info_json)" || return 1
  AOC_ZELLIJ_PANES_JSON="$panes_json" \
  AOC_ZELLIJ_CURRENT_TAB_JSON="$current_tab_json" \
  python3 - "$target_name" <<'PY'
import json, os, sys

target_name = sys.argv[1]
panes_raw = os.environ["AOC_ZELLIJ_PANES_JSON"]
current_raw = os.environ["AOC_ZELLIJ_CURRENT_TAB_JSON"]

def load(raw):
    val = json.loads(raw)
    if isinstance(val, list):
        return val
    if isinstance(val, dict):
        for key in ("panes", "data", "items"):
            child = val.get(key)
            if isinstance(child, list):
                return child
    return []

def load_obj(raw):
    val = json.loads(raw)
    if isinstance(val, dict):
        for key in ("tab", "data", "current_tab"):
            child = val.get(key)
            if isinstance(child, dict):
                return child
        return val
    return {}

def first(obj, *keys):
    for key in keys:
        value = obj.get(key)
        if value not in (None, ""):
            return value
    return None

current = load_obj(current_raw)
current_tab_id = first(current, "tab_id", "id", "tabId")
current_index = first(current, "position", "index", "tab_index", "tabPosition")

for pane in load(panes_raw):
    if not isinstance(pane, dict):
        continue
    tab_id = first(pane, "tab_id", "tabId")
    tab_index = first(pane, "tab_position", "tab_index", "position")
    if current_tab_id is not None and tab_id is not None and str(tab_id) != str(current_tab_id):
        continue
    if current_tab_id is None and current_index is not None and tab_index is not None and str(tab_index) != str(current_index):
        continue
    name = first(pane, "name", "pane_name", "title")
    if str(name) != target_name:
        continue
    pane_id = first(pane, "id", "pane_id", "paneId")
    if pane_id not in (None, ""):
        print(pane_id)
        raise SystemExit(0)
raise SystemExit(1)
PY
}

aoc_zellij_dump_screen() {
  local pane_id="$1"
  local session_id="${2:-${ZELLIJ_SESSION_NAME:-${AOC_SESSION_ID:-}}}"
  [[ -n "$pane_id" ]] || return 2
  _aoc_zellij_supports_action dump-screen || return 1
  local cmd=(zellij)
  if [[ -n "$session_id" ]]; then
    cmd+=(--session "$session_id")
  fi
  cmd+=(action dump-screen --pane-id "$pane_id" --full --ansi)
  "${cmd[@]}"
}

aoc_zellij_subscribe_pane() {
  local pane_id="$1"
  local session_id="${2:-${ZELLIJ_SESSION_NAME:-${AOC_SESSION_ID:-}}}"
  local scrollback="${3:-200}"
  [[ -n "$pane_id" ]] || return 2
  command -v zellij >/dev/null 2>&1 || return 1
  local cmd=(zellij)
  if [[ -n "$session_id" ]]; then
    cmd+=(--session "$session_id")
  fi
  cmd+=(subscribe --pane-id "$pane_id" --format json --scrollback "$scrollback" --ansi)
  "${cmd[@]}"
}

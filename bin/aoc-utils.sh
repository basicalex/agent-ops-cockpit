#!/usr/bin/env bash

generate_whimsical_session_id() {
  local nouns=(
    otter badger alpaca ferret penguin walrus narwhal yak koala gecko
    capybara platypus lemur puffin iguana beaver hedgehog squirrel raccoon
    aardvark wombat lynx mojito noodle waffle pixel
  )
  local verbs=(
    juggles debugs compiles refactors lints ships rebases squashes syncs
    profiles benches deploys patches bisects tests tidies wrangles explores
    orchestrates untangles snapshots autopilots
  )
  local noun="${nouns[RANDOM % ${#nouns[@]}]}"
  local verb="${verbs[RANDOM % ${#verbs[@]}]}"
  printf '%s-%s' "$noun" "$verb"
}

session_name_exists() {
  local candidate="$1"
  if ! command -v zellij >/dev/null 2>&1; then
    return 1
  fi
  zellij list-sessions --short --no-formatting 2>/dev/null | grep -Fxq "$candidate"
}

derive_port() {
  local session="$1"
  if ! command -v python3 >/dev/null 2>&1; then
    printf '42000'
    return
  fi
  python3 - <<'PY' "$session"
import sys

session = sys.argv[1].encode()
hash_value = 2166136261
for byte in session:
    hash_value ^= byte
    hash_value = (hash_value * 16777619) & 0xFFFFFFFF
port = 42000 + (hash_value % 2000)
print(port)
PY
}

resolve_hub_addr() {
  local session_id="$1"
  if [[ -n "${AOC_HUB_ADDR:-}" ]]; then
    printf '%s' "$AOC_HUB_ADDR"
    return
  fi
  local port
  port="$(derive_port "$session_id")"
  printf '127.0.0.1:%s' "$port"
}

resolve_hub_url() {
  local hub_addr="$1"
  if [[ -n "${AOC_HUB_URL:-}" ]]; then
    printf '%s' "$AOC_HUB_URL"
    return
  fi
  printf 'ws://%s/ws' "$hub_addr"
}

sanitize_name() {
  local raw="$1"
  raw="$(printf '%s' "$raw" | tr '[:upper:]' '[:lower:]')"
  raw="$(printf '%s' "$raw" | sed -E 's/[^a-z0-9]+/-/g; s/^-+|-+$//g')"
  printf '%s' "${raw:-tab}"
}

hub_health_ok() {
  local addr="$1"
  if command -v curl >/dev/null 2>&1; then
    local body
    body="$(curl -fsS --max-time 1 "http://$addr/health" 2>/dev/null || true)"
    [[ "$body" == "ok" ]]
    return
  fi
  if command -v python3 >/dev/null 2>&1; then
    python3 - <<'PY' "$addr"
import sys
import urllib.request

url = f"http://{sys.argv[1]}/health"
try:
    with urllib.request.urlopen(url, timeout=1) as resp:
        body = resp.read().decode("utf-8", errors="ignore").strip()
        if body == "ok":
            raise SystemExit(0)
except Exception:
    pass
raise SystemExit(1)
PY
    return
  fi
  return 1
}

ensure_hub_running() {
  local session_id="$1"
  local hub_addr="$2"
  local state_root="$3"
  local session_slug
  session_slug="$(sanitize_name "$session_id")"
  local pid_file="$state_root/hub-${session_slug}.pid"
  local log_file="$state_root/hub-${session_slug}.log"
  local lock_file="/tmp/aoc-hub-${session_slug}.lock"

  _aoc_start_hub_unlocked() {
    if hub_health_ok "$hub_addr"; then
      return
    fi

    if [[ -f "$pid_file" ]]; then
      local existing_pid
      existing_pid="$(cat "$pid_file" 2>/dev/null || true)"
      if [[ -n "$existing_pid" ]] && kill -0 "$existing_pid" 2>/dev/null; then
        local i
        for i in 1 2 3 4 5; do
          if hub_health_ok "$hub_addr"; then
            return
          fi
          sleep 0.2
        done
      fi
      rm -f "$pid_file"
    fi

    AOC_SESSION_ID="$session_id" AOC_HUB_ADDR="$hub_addr" nohup aoc-hub >>"$log_file" 2>&1 &
    local hub_pid=$!
    printf '%s\n' "$hub_pid" > "$pid_file"

    local i
    for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
      if hub_health_ok "$hub_addr"; then
        return
      fi
      sleep 0.2
    done
  }

  if hub_health_ok "$hub_addr"; then
    return
  fi

  if ! command -v aoc-hub >/dev/null 2>&1; then
    return
  fi

  if command -v flock >/dev/null 2>&1; then
    (
      exec 9> "$lock_file"
      if flock -w 5 9; then
        _aoc_start_hub_unlocked
      else
        local i
        for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25; do
          if hub_health_ok "$hub_addr"; then
            return
          fi
          sleep 0.2
        done
      fi
    )
    return
  fi

  _aoc_start_hub_unlocked
}

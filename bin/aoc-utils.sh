#!/usr/bin/env bash


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



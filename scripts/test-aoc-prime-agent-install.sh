#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

script="$root/bin/aoc-prime-agent-install"

assert_eq() {
  local actual="$1"
  local expected="$2"
  if [[ "$actual" != "$expected" ]]; then
    printf 'expected %q, got %q\n' "$expected" "$actual" >&2
    exit 1
  fi
}

make_prime_agent() {
  local dir="$1"
  mkdir -p "$dir"
  cat >"$dir/prime-agent" <<'EOF'
#!/usr/bin/env bash
printf 'fake prime-agent\n'
EOF
  chmod 0755 "$dir/prime-agent"
}

# Existing prime-agent is a no-op and does not fetch.
noop_home="$tmp/noop-home"
noop_path="$tmp/noop-bin"
mkdir -p "$noop_home" "$noop_path"
make_prime_agent "$noop_path"
cat >"$noop_path/curl" <<'EOF'
#!/usr/bin/env bash
printf 'curl should not be called when prime-agent exists\n' >&2
exit 42
EOF
chmod 0755 "$noop_path/curl"
HOME="$noop_home" PATH="$noop_path:/usr/bin:/bin" bash "$script"
assert_eq "$(HOME="$noop_home" PATH="$noop_path:/usr/bin:/bin" bash "$script" --status)" "installed"

# Missing status reports missing without installing.
missing_home="$tmp/missing-home"
missing_path="$tmp/missing-bin"
mkdir -p "$missing_home" "$missing_path"
assert_eq "$(HOME="$missing_home" PATH="$missing_path:/usr/bin:/bin" bash "$script" --status)" "missing"

# PRIME_AGENT_INSTALL_URL can point at a deterministic installer, and verification
# finds the command once the installer drops it in ~/.local/bin.
fake_home="$tmp/fake-home"
fake_path="$tmp/fake-bin"
fake_installer="$tmp/install-prime-agent.sh"
mkdir -p "$fake_home" "$fake_path"
cat >"$fake_path/curl" <<'EOF'
#!/usr/bin/env bash
case "$1 $2" in
  "-fsSL file://"*) cat "${2#file://}" ;;
  *) printf 'unexpected curl args: %s\n' "$*" >&2; exit 2 ;;
esac
EOF
chmod 0755 "$fake_path/curl"
cat >"$fake_installer" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "${DO_NOT_TRACK:-}" == "1" ]]
[[ "${PRIME_AGENT_INSTALLER_PLAIN:-}" == "1" ]]
[[ "${PRIME_AGENT_BOOTSTRAP_KERNEL_ON_INSTALL:-}" == "1" ]]
mkdir -p "$HOME/.local/bin"
cat >"$HOME/.local/bin/prime-agent" <<'AGENT'
#!/usr/bin/env bash
printf 'fake prime-agent\n'
AGENT
chmod 0755 "$HOME/.local/bin/prime-agent"
EOF
HOME="$fake_home" PATH="$fake_path:/usr/bin:/bin" PRIME_AGENT_INSTALL_URL="file://$fake_installer" bash "$script"
assert_eq "$(HOME="$fake_home" PATH="$fake_path:/usr/bin:/bin" bash "$script" --status)" "installed"

# --force reinstalls even when prime-agent already resolves.
HOME="$fake_home" PATH="$fake_path:/usr/bin:/bin" PRIME_AGENT_INSTALL_URL="file://$fake_installer" bash "$script" --force
assert_eq "$(HOME="$fake_home" PATH="$fake_path:/usr/bin:/bin" bash "$script" --status)" "installed"

# An installer that drops the command somewhere unreachable must fail loudly.
broken_home="$tmp/broken-home"
broken_path="$tmp/broken-bin"
broken_installer="$tmp/install-broken.sh"
mkdir -p "$broken_home" "$broken_path"
cp "$fake_path/curl" "$broken_path/curl"
cat >"$broken_installer" <<'EOF'
#!/usr/bin/env bash
printf 'pretending to install\n'
EOF
if HOME="$broken_home" PATH="$broken_path:/usr/bin:/bin" \
  PRIME_AGENT_INSTALL_URL="file://$broken_installer" bash "$script" 2>/dev/null; then
  printf 'expected failure when prime-agent is not on PATH after install\n' >&2
  exit 1
fi

# Syntax checks for the installer integration surface.
bash -n "$script"
bash -n "$root/install.sh"
bash -n "${BASH_SOURCE[0]}"

printf 'AOC prime-agent installer checks passed\n'

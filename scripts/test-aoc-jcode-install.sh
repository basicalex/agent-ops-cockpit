#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

script="$root/bin/aoc-jcode-install"

assert_eq() {
  local actual="$1"
  local expected="$2"
  if [[ "$actual" != "$expected" ]]; then
    printf 'expected %q, got %q\n' "$expected" "$actual" >&2
    exit 1
  fi
}

make_jcode() {
  local dir="$1"
  mkdir -p "$dir"
  cat >"$dir/jcode" <<'EOF'
#!/usr/bin/env bash
printf 'fake jcode\n'
EOF
  chmod 0755 "$dir/jcode"
}

# Existing jcode is a no-op and does not fetch.
noop_home="$tmp/noop-home"
noop_path="$tmp/noop-bin"
mkdir -p "$noop_home" "$noop_path"
make_jcode "$noop_path"
cat >"$noop_path/curl" <<'EOF'
#!/usr/bin/env bash
printf 'curl should not be called when jcode exists\n' >&2
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

# JCODE_INSTALL_URL can point at a deterministic installer, and verification finds ~/.local/bin.
fake_home="$tmp/fake-home"
fake_path="$tmp/fake-bin"
fake_installer="$tmp/install-jcode.sh"
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
[[ "${JCODE_NO_TELEMETRY:-}" == "1" ]]
mkdir -p "$HOME/.local/bin"
cat >"$HOME/.local/bin/jcode" <<'JCODE'
#!/usr/bin/env bash
printf 'fake jcode\n'
JCODE
chmod 0755 "$HOME/.local/bin/jcode"
EOF
HOME="$fake_home" PATH="$fake_path:/usr/bin:/bin" JCODE_INSTALL_URL="file://$fake_installer" bash "$script"
assert_eq "$(HOME="$fake_home" PATH="$fake_path:/usr/bin:/bin" bash "$script" --status)" "installed"

# Syntax checks for the installer integration surface.
bash -n "$script"
bash -n "$root/install.sh"
bash -n "${BASH_SOURCE[0]}"

printf 'AOC jcode installer checks passed\n'

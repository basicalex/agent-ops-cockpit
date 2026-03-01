#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$HOME/.local/bin"

# Ensure user-local bin is discoverable during install checks
export PATH="$BIN_DIR:$PATH"

# Ensure dirs exist
mkdir -p "$BIN_DIR"
mkdir -p "$HOME/.config/zellij/layouts"
mkdir -p "$HOME/.config/zellij"
mkdir -p "$HOME/.config/zellij/plugins"
mkdir -p "$HOME/.config/yazi"
mkdir -p "$HOME/.config/yazi/plugins"
mkdir -p "${MICRO_CONFIG_HOME:-${XDG_CONFIG_HOME:-$HOME/.config}/micro}"
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/aoc"
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/btop"
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/skills"
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/taskmaster/templates"
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/prompts/pi"
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/skills-optional"
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/prompts-optional/pi"
mkdir -p "${XDG_STATE_HOME:-$HOME/.local/state}/aoc"

log() { echo ">> $1"; }
warn() { echo "!! $1"; }
have() { command -v "$1" >/dev/null 2>&1; }
have_bat() { have bat || have batcat; }
is_truthy() {
  local value="${1:-}"
  value="$(printf '%s' "$value" | tr '[:upper:]' '[:lower:]')"
  case "$value" in
    1|true|yes|on) return 0 ;;
    *) return 1 ;;
  esac
}

install_rust_toolchain_if_needed() {
  if cargo_cmd >/dev/null 2>&1; then
    return 0
  fi

  if ! is_truthy "${AOC_INSTALL_RUST:-1}"; then
    warn "cargo missing and AOC_INSTALL_RUST=0; Rust components may be unavailable."
    return 1
  fi

  log "cargo missing; installing Rust toolchain via rustup..."
  if have curl; then
    if ! curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable; then
      warn "rustup install failed via curl."
      return 1
    fi
  elif have wget; then
    if ! wget -qO- https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable; then
      warn "rustup install failed via wget."
      return 1
    fi
  else
    warn "curl or wget required for rustup bootstrap."
    return 1
  fi

  export PATH="$HOME/.cargo/bin:$PATH"
  if cargo_cmd >/dev/null 2>&1; then
    return 0
  fi

  warn "cargo still unavailable after rustup install."
  return 1
}

install_pi_agent_if_enabled() {
  if ! is_truthy "${AOC_INSTALL_PI_AGENT:-1}"; then
    log "Skipping PI agent install (set AOC_INSTALL_PI_AGENT=1 to enable)."
    return 0
  fi

  local pi_bin="${AOC_PI_BIN:-pi}"
  if have "$pi_bin"; then
    log "PI agent already installed ($pi_bin)."
    return 0
  fi

  if ! have pnpm && ! have npm && ! have corepack; then
    log "pnpm/npm missing; attempting Node.js install..."
    install_tool npm || true
  fi

  if ! have pnpm && have corepack; then
    corepack enable >/dev/null 2>&1 || true
    corepack prepare pnpm@latest --activate >/dev/null 2>&1 || true
  fi

  local installer=""
  if [[ -x "$BIN_DIR/aoc-agent-install" ]]; then
    installer="$BIN_DIR/aoc-agent-install"
  elif have aoc-agent-install; then
    installer="$(command -v aoc-agent-install)"
  elif [[ -x "$ROOT_DIR/bin/aoc-agent-install" ]]; then
    installer="$ROOT_DIR/bin/aoc-agent-install"
  fi

  if [[ -z "$installer" ]]; then
    warn "aoc-agent-install not found; cannot install PI agent automatically."
    return 1
  fi

  log "Installing PI agent CLI..."
  if ! "$installer" install pi; then
    warn "PI agent install command failed."
    return 1
  fi

  if have "$pi_bin"; then
    log "PI agent installed ($pi_bin)."
    return 0
  fi

  warn "PI agent install completed but '$pi_bin' is still missing from PATH."
  return 1
}

install_omo_if_enabled() {
  if ! is_truthy "${AOC_INSTALL_OMO:-0}"; then
    log "Skipping OmO install (set AOC_INSTALL_OMO=1 to enable)."
    return
  fi

  local script_path="$ROOT_DIR/scripts/opencode/install-omo.sh"
  if [[ ! -f "$script_path" ]]; then
    warn "OmO installer wrapper not found at $script_path"
    if is_truthy "${AOC_INSTALL_OMO_REQUIRED:-0}"; then
      exit 1
    fi
    return
  fi

  local profile_name="${AOC_OMO_PROFILE:-sandbox}"
  local claude="${AOC_OMO_CLAUDE:-no}"
  local openai="${AOC_OMO_OPENAI:-no}"
  local gemini="${AOC_OMO_GEMINI:-no}"
  local copilot="${AOC_OMO_COPILOT:-no}"
  local opencode_zen="${AOC_OMO_OPENCODE_ZEN:-no}"
  local zai_coding_plan="${AOC_OMO_ZAI_CODING_PLAN:-no}"

  log "Installing OmO into OpenCode profile '$profile_name'..."
  if ! bash "$script_path" install \
    --profile "$profile_name" \
    --claude "$claude" \
    --openai "$openai" \
    --gemini "$gemini" \
    --copilot "$copilot" \
    --opencode-zen "$opencode_zen" \
    --zai-coding-plan "$zai_coding_plan"; then
    warn "OmO install failed for profile '$profile_name'."
    if is_truthy "${AOC_INSTALL_OMO_REQUIRED:-0}"; then
      exit 1
    fi
  fi
}

github_token() {
  if [[ -n "${AOC_GITHUB_TOKEN:-}" ]]; then
    printf '%s\n' "$AOC_GITHUB_TOKEN"
    return
  fi
  if [[ -n "${GITHUB_TOKEN:-}" ]]; then
    printf '%s\n' "$GITHUB_TOKEN"
    return
  fi
  if [[ -n "${GH_TOKEN:-}" ]]; then
    printf '%s\n' "$GH_TOKEN"
    return
  fi
  printf '\n'
}

is_github_api_url() {
  [[ "$1" == https://api.github.com/* ]]
}

fetch_stdout() {
  local url="$1"
  local token
  token="$(github_token)"
  if have curl; then
    local curl_args=(-fsSL --retry 5 --retry-delay 2 --retry-all-errors --connect-timeout 20 --max-time 300)
    if [[ -n "$token" ]] && is_github_api_url "$url"; then
      curl "${curl_args[@]}" \
        -H "Authorization: Bearer $token" \
        -H "X-GitHub-Api-Version: 2022-11-28" \
        "$url"
    else
      curl "${curl_args[@]}" "$url"
    fi
  elif have wget; then
    local wget_args=(--tries=5 --waitretry=2 --retry-connrefused --timeout=30 -qO-)
    if [[ -n "$token" ]] && is_github_api_url "$url"; then
      wget "${wget_args[@]}" \
        --header="Authorization: Bearer $token" \
        --header="X-GitHub-Api-Version: 2022-11-28" \
        "$url"
    else
      wget "${wget_args[@]}" "$url"
    fi
  else
    return 1
  fi
}

download_to_file() {
  local url="$1"
  local out="$2"
  local token
  token="$(github_token)"
  if have curl; then
    local curl_args=(-fsSL --retry 5 --retry-delay 2 --retry-all-errors --connect-timeout 20 --max-time 300 -o "$out")
    if [[ -n "$token" ]] && is_github_api_url "$url"; then
      curl "${curl_args[@]}" \
        -H "Authorization: Bearer $token" \
        -H "X-GitHub-Api-Version: 2022-11-28" \
        "$url"
    else
      curl "${curl_args[@]}" "$url"
    fi
  elif have wget; then
    local wget_args=(--tries=5 --waitretry=2 --retry-connrefused --timeout=30 -qO "$out")
    if [[ -n "$token" ]] && is_github_api_url "$url"; then
      wget "${wget_args[@]}" \
        --header="Authorization: Bearer $token" \
        --header="X-GitHub-Api-Version: 2022-11-28" \
        "$url"
    else
      wget "${wget_args[@]}" "$url"
    fi
  else
    return 1
  fi
}

latest_release_tag_from_redirect() {
  local repo="$1"
  local resolved tag

  if ! have curl; then
    return 1
  fi

  resolved="$(curl -fsSL --retry 5 --retry-delay 2 --retry-all-errors \
    -o /dev/null -w '%{url_effective}' "https://github.com/${repo}/releases/latest")" || return 1
  resolved="${resolved%%\?*}"
  if [[ "$resolved" != *"/releases/tag/"* ]]; then
    return 1
  fi

  tag="${resolved##*/}"
  [[ -n "$tag" ]] || return 1
  printf '%s\n' "$tag"
}

latest_release_tag() {
  local repo="$1"
  local api="https://api.github.com/repos/${repo}/releases/latest"
  local json tag=""

  if json="$(fetch_stdout "$api" 2>/dev/null)"; then
    tag="$(printf '%s\n' "$json" | awk -F'"' '/"tag_name"[[:space:]]*:/ {print $4; exit}')"
  fi

  if [[ -z "$tag" ]]; then
    tag="$(latest_release_tag_from_redirect "$repo" 2>/dev/null || true)"
  fi

  [[ -n "$tag" ]] || return 1
  printf '%s\n' "$tag"
}

linux_arch_target() {
  local arch
  arch="$(uname -m)"
  case "$arch" in
    x86_64) printf 'x86_64' ;;
    aarch64|arm64) printf 'aarch64' ;;
    *) printf '' ;;
  esac
}

install_zellij_binary() {
  [[ "$(uname -s)" == "Linux" ]] || return 1
  local arch tag tmpdir asset url
  arch="$(linux_arch_target)"
  [[ -n "$arch" ]] || return 1
  tag="$(latest_release_tag "zellij-org/zellij")" || return 1

  tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/aoc-zellij.XXXXXX")"
  asset="zellij-${arch}-unknown-linux-musl.tar.gz"
  url="https://github.com/zellij-org/zellij/releases/download/${tag}/${asset}"

  if ! download_to_file "$url" "$tmpdir/$asset"; then
    rm -rf "$tmpdir"
    return 1
  fi

  if ! tar -xzf "$tmpdir/$asset" -C "$tmpdir"; then
    rm -rf "$tmpdir"
    return 1
  fi

  if [[ -f "$tmpdir/zellij" ]]; then
    install -m 0755 "$tmpdir/zellij" "$BIN_DIR/zellij"
    rm -rf "$tmpdir"
    return 0
  fi

  rm -rf "$tmpdir"
  return 1
}

install_yazi_binary() {
  [[ "$(uname -s)" == "Linux" ]] || return 1
  local arch tag tmpdir asset url
  arch="$(linux_arch_target)"
  [[ -n "$arch" ]] || return 1
  tag="$(latest_release_tag "sxyazi/yazi")" || return 1

  if ! have unzip; then
    pm_install unzip || return 1
  fi

  tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/aoc-yazi.XXXXXX")"
  asset="yazi-${arch}-unknown-linux-musl.zip"
  url="https://github.com/sxyazi/yazi/releases/download/${tag}/${asset}"

  if ! download_to_file "$url" "$tmpdir/$asset"; then
    asset="yazi-${arch}-unknown-linux-gnu.zip"
    url="https://github.com/sxyazi/yazi/releases/download/${tag}/${asset}"
    if ! download_to_file "$url" "$tmpdir/$asset"; then
      rm -rf "$tmpdir"
      return 1
    fi
  fi

  if ! unzip -q "$tmpdir/$asset" -d "$tmpdir"; then
    rm -rf "$tmpdir"
    return 1
  fi

  local yazi_bin ya_bin
  yazi_bin="$(find "$tmpdir" -type f -name yazi 2>/dev/null | head -n1)"
  ya_bin="$(find "$tmpdir" -type f -name ya 2>/dev/null | head -n1)"

  if [[ -n "$yazi_bin" && -f "$yazi_bin" ]]; then
    install -m 0755 "$yazi_bin" "$BIN_DIR/yazi"
  else
    rm -rf "$tmpdir"
    return 1
  fi

  if [[ -n "$ya_bin" && -f "$ya_bin" ]]; then
    install -m 0755 "$ya_bin" "$BIN_DIR/ya"
  fi

  rm -rf "$tmpdir"
  return 0
}

cargo_cmd() {
  if [[ -x "$HOME/.cargo/bin/cargo" ]]; then
    echo "$HOME/.cargo/bin/cargo"
    return 0
  fi
  if command -v cargo >/dev/null 2>&1; then
    command -v cargo
    return 0
  fi
  return 1
}

cargo_with_retry() {
  local max_attempts="${AOC_CARGO_RETRIES:-3}"
  local delay_secs="${AOC_CARGO_RETRY_DELAY_SECS:-5}"
  local attempt=1

  while true; do
    if "$@"; then
      return 0
    fi
    if (( attempt >= max_attempts )); then
      return 1
    fi
    warn "Cargo command failed (attempt ${attempt}/${max_attempts}); retrying in ${delay_secs}s..."
    sleep "$delay_secs"
    attempt=$((attempt + 1))
  done
}

detect_pm() {
  if have apt-get; then echo "apt"; return; fi
  if have dnf; then echo "dnf"; return; fi
  if have pacman; then echo "pacman"; return; fi
  if have brew; then echo "brew"; return; fi
  if have apk; then echo "apk"; return; fi
  if have yum; then echo "yum"; return; fi
  if have zypper; then echo "zypper"; return; fi
  echo "unknown"
}

run_root() {
  if [[ "$EUID" -eq 0 ]]; then
    "$@"
    return
  fi
  if have sudo; then
    sudo "$@"
    return
  fi
  warn "sudo not available; run as root to install packages."
  return 1
}

pm_install() {
  local pkgs=("$@")
  if ((${#pkgs[@]} == 0)); then
    return 0
  fi
  case "$pm" in
    apt)
      if [[ "$apt_updated" -eq 0 ]]; then
        if ! run_root apt-get update; then
          return 1
        fi
        apt_updated=1
      fi
      if ! run_root apt-get install -y "${pkgs[@]}"; then
        return 1
      fi
      ;;
    dnf)
      if ! run_root dnf install -y "${pkgs[@]}"; then
        return 1
      fi
      ;;
    pacman)
      if ! run_root pacman -S --noconfirm --needed "${pkgs[@]}"; then
        return 1
      fi
      ;;
    brew)
      if ! brew install "${pkgs[@]}"; then
        return 1
      fi
      ;;
    apk)
      if ! run_root apk add "${pkgs[@]}"; then
        return 1
      fi
      ;;
    yum)
      if ! run_root yum install -y "${pkgs[@]}"; then
        return 1
      fi
      ;;
    zypper)
      if ! run_root zypper install -y "${pkgs[@]}"; then
        return 1
      fi
      ;;
    *)
      warn "No supported package manager found for installing: ${pkgs[*]}"
      return 1
      ;;
  esac
}

ensure_rust_build_prereqs() {
  if have cc && have pkg-config; then
    return 0
  fi

  case "$pm" in
    apt)
      pm_install build-essential pkg-config libssl-dev
      ;;
    dnf)
      pm_install gcc gcc-c++ make pkgconf-pkg-config openssl-devel
      ;;
    pacman)
      pm_install base-devel pkgconf openssl
      ;;
    apk)
      pm_install build-base pkgconf openssl-dev
      ;;
    yum)
      pm_install gcc gcc-c++ make pkgconfig openssl-devel
      ;;
    zypper)
      pm_install gcc gcc-c++ make pkg-config libopenssl-devel
      ;;
    brew)
      # Assume Command Line Tools are present if cc/pkg-config already exist.
      ;;
    *)
      warn "Unknown package manager; cannot auto-install Rust build prerequisites."
      ;;
  esac

  if have cc && have pkg-config; then
    return 0
  fi

  warn "Rust build prerequisites missing (need cc and pkg-config)."
  return 1
}

install_tool() {
  local tool="$1"
  local cargo_bin=""
  case "$tool" in
    curl|wget)
      pm_install "$tool" || warn "Failed to install $tool."
      ;;
    zellij)
      pm_install zellij || warn "Failed to install zellij via package manager."
      if ! have zellij; then
        if install_zellij_binary; then
          return
        fi
        if cargo_bin="$(cargo_cmd)"; then
          log "Installing zellij via cargo..."
          "$cargo_bin" install --locked zellij || warn "Failed to install zellij via cargo."
        fi
      fi
      ;;
    yazi)
      case "$pm" in
        brew|pacman|dnf|apk|apt|yum|zypper)
          pm_install yazi || warn "Failed to install yazi via package manager."
          ;;
        *)
          ;;
      esac
      if ! have yazi; then
        if install_yazi_binary; then
          return
        fi
        if cargo_bin="$(cargo_cmd)"; then
          log "Installing yazi via cargo (yazi-build)..."
          if ! "$cargo_bin" install --locked --force yazi-build; then
            warn "Failed to install yazi via yazi-build."
            log "Attempting legacy yazi-fm/yazi-cli install..."
            "$cargo_bin" install --locked yazi-fm yazi-cli || warn "Failed to install yazi via cargo."
          fi
        fi
      fi
      ;;
    npm|node|nodejs)
      case "$pm" in
        apt|dnf|apk|yum|zypper)
          pm_install nodejs npm || warn "Failed to install nodejs/npm."
          ;;
        pacman)
          pm_install nodejs npm || warn "Failed to install nodejs/npm."
          ;;
        brew)
          pm_install node || warn "Failed to install node via Homebrew."
          ;;
        *)
          warn "No package manager mapping for nodejs/npm."
          ;;
      esac
      ;;
    pnpm)
      case "$pm" in
        brew|pacman|dnf|apk)
          pm_install pnpm || warn "Failed to install pnpm via package manager."
          ;;
        *)
          ;;
      esac

      if ! have pnpm && have corepack; then
        corepack enable >/dev/null 2>&1 || true
        corepack prepare pnpm@latest --activate >/dev/null 2>&1 || true
      fi

      if ! have pnpm && have npm; then
        npm install -g --prefix "${AOC_NPM_GLOBAL_PREFIX:-$HOME/.local}" pnpm || warn "Failed to install pnpm via npm."
      fi
      ;;
    fzf|tmux|chafa|ffmpeg)
      pm_install "$tool" || warn "Failed to install $tool."
      ;;
    pdftoppm)
      case "$pm" in
        apt|dnf|apk|yum)
          pm_install poppler-utils || warn "Failed to install poppler-utils."
          ;;
        pacman|brew)
          pm_install poppler || warn "Failed to install poppler."
          ;;
        zypper)
          pm_install poppler-tools || warn "Failed to install poppler-tools."
          ;;
        *)
          warn "No package manager mapping for poppler."
          ;;
      esac
      ;;
    rsvg-convert)
      case "$pm" in
        apt)
          pm_install librsvg2-bin || warn "Failed to install librsvg2-bin."
          ;;
        dnf)
          pm_install librsvg2-tools || warn "Failed to install librsvg2-tools."
          ;;
        pacman|brew|apk|yum|zypper)
          pm_install librsvg || warn "Failed to install librsvg."
          ;;
        *)
          warn "No package manager mapping for librsvg."
          ;;
      esac
      ;;
    rg)
      pm_install ripgrep || warn "Failed to install ripgrep."
      ;;
    bat)
      pm_install bat || warn "Failed to install bat."
      ;;
  esac
}

ensure_tool() {
  local tool="$1"
  local label="${2:-$tool}"
  if have "$tool"; then
    return 0
  fi
  log "$label missing; installing..."
  install_tool "$tool"
  if have "$tool"; then
    return 0
  fi
  warn "$label still missing."
  return 1
}

ensure_bat() {
  if have_bat; then
    return 0
  fi
  log "bat missing; installing..."
  install_tool bat
  if have_bat; then
    return 0
  fi
  warn "bat still missing."
  return 1
}

pm=""
apt_updated=0

# 1. Install Scripts
log "Installing scripts..."
for f in "$ROOT_DIR/bin/"*; do
  filename=$(basename "$f")
  # Skip micro if it's there (it shouldn't be, but just in case)
  [[ "$filename" == "micro" ]] && continue

  install -m 0755 "$f" "$BIN_DIR/$filename"
done

required_bin_scripts=(
  aoc
  aoc-launch
  aoc-new-tab
  aoc-agent-wrap
  aoc-utils.sh
  aoc-init
  aoc-doctor
  tm
)
missing_installed_scripts=()
for script_name in "${required_bin_scripts[@]}"; do
  if [[ ! -f "$BIN_DIR/$script_name" ]]; then
    missing_installed_scripts+=("$script_name")
  fi
done

if ((${#missing_installed_scripts[@]} > 0)); then
  warn "Script install incomplete; missing in $BIN_DIR: ${missing_installed_scripts[*]}"
  exit 1
fi

# Remove retired non-PI wrappers from previous installs.
retired_prefixed_wrappers=(
  aoc-codex
  aoc-gemini
  aoc-cc
  aoc-oc
  aoc-kimi
  aoc-omo
  aoc-codex-tab
  aoc-opencode-profile
)
retired_legacy_aliases=(
  codex
  gemini
  claude
  opencode
  kimi
)

remove_if_pi_deprecation_stub() {
  local target="$1"
  [[ -f "$target" ]] || return 0
  if grep -Fq "removed in PI-only mode" "$target" 2>/dev/null; then
    rm -f "$target"
  fi
}

for wrapper in "${retired_prefixed_wrappers[@]}"; do
  rm -f "$BIN_DIR/$wrapper"
done
for wrapper in "${retired_legacy_aliases[@]}"; do
  remove_if_pi_deprecation_stub "$BIN_DIR/$wrapper"
done

if [[ -d "$HOME/bin" && -w "$HOME/bin" ]]; then
  for wrapper in "${retired_prefixed_wrappers[@]}"; do
    rm -f "$HOME/bin/$wrapper"
  done
  for wrapper in "${retired_legacy_aliases[@]}"; do
    remove_if_pi_deprecation_stub "$HOME/bin/$wrapper"
  done
fi

# 2. Rust Build & Install
pm="$(detect_pm)"
if [[ "$pm" == "unknown" ]]; then
  warn "No supported package manager found; dependency installs may be limited."
fi

log "Building Rust components..."
if ! cargo_cmd >/dev/null 2>&1; then
  install_rust_toolchain_if_needed || warn "Continuing without cargo; Rust binaries may be unavailable."
fi

if cargo_cmd >/dev/null 2>&1; then
  if ! ensure_rust_build_prereqs; then
    warn "Cannot build Rust components without C toolchain prerequisites."
    exit 1
  fi
fi

cargo_bin=""
if cargo_bin="$(cargo_cmd)"; then
  if [[ "$cargo_bin" == "$HOME/.cargo/bin/cargo" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
  export CARGO_NET_RETRY="${CARGO_NET_RETRY:-5}"
  export CARGO_HTTP_TIMEOUT="${CARGO_HTTP_TIMEOUT:-120}"

  cargo_version="$($cargo_bin --version | awk '{print $2}')"
  cargo_major="${cargo_version%%.*}"
  cargo_minor="${cargo_version#*.}"
  cargo_minor="${cargo_minor%%.*}"
  lockfile="$ROOT_DIR/crates/Cargo.lock"
  if [[ "$cargo_major" -eq 1 && "$cargo_minor" -lt 78 ]]; then
    if [[ -f "$lockfile" ]] && grep -q '^version = 4' "$lockfile"; then
      log "Downgrading lockfile for Cargo $cargo_version..."
      rm -f "$lockfile"
      "$cargo_bin" generate-lockfile --manifest-path "$ROOT_DIR/crates/Cargo.toml"
    fi
  fi

  log "Resolving Rust dependencies (first run may take a few minutes)..."
  if ! cargo_with_retry "$cargo_bin" fetch --manifest-path "$ROOT_DIR/crates/Cargo.toml"; then
    warn "Cargo fetch failed; continuing and letting build/install attempt recover."
  fi

  # Build aoc-cli
  log "Building aoc-cli..."
  cargo_with_retry "$cargo_bin" install --path "$ROOT_DIR/crates/aoc-cli" --root "$HOME/.local" --force || {
    # Fallback for older cargos that don't support --root in the same way or if it fails
    # Try direct build
    log "Cargo install failed, trying build --release..."
    (cd "$ROOT_DIR/crates" && "$cargo_bin" build --release -p aoc-cli)
    cp "$ROOT_DIR/crates/target/release/aoc-cli" "$BIN_DIR/aoc-cli"
  }

  # Build aoc-taskmaster (native TUI)
  log "Building aoc-taskmaster..."
  if "$cargo_bin" build --release -p aoc-taskmaster --manifest-path "$ROOT_DIR/crates/Cargo.toml"; then
    if [[ -f "$ROOT_DIR/crates/target/release/aoc-taskmaster" ]]; then
      install -m 0755 "$ROOT_DIR/crates/target/release/aoc-taskmaster" "$BIN_DIR/aoc-taskmaster-native"
    fi
  else
    log "WARNING: Failed to build aoc-taskmaster."
  fi

  # Build aoc-control (native TUI)
  log "Building aoc-control..."
  if "$cargo_bin" build --release -p aoc-control --manifest-path "$ROOT_DIR/crates/Cargo.toml"; then
    if [[ -f "$ROOT_DIR/crates/target/release/aoc-control" ]]; then
      install -m 0755 "$ROOT_DIR/crates/target/release/aoc-control" "$BIN_DIR/aoc-control-native"
    fi
  else
    log "WARNING: Failed to build aoc-control."
  fi

  # Build aoc-mission-control (native TUI)
  log "Building aoc-mission-control..."
  if "$cargo_bin" build --release -p aoc-mission-control --manifest-path "$ROOT_DIR/crates/Cargo.toml"; then
    if [[ -f "$ROOT_DIR/crates/target/release/aoc-mission-control" ]]; then
      install -m 0755 "$ROOT_DIR/crates/target/release/aoc-mission-control" "$BIN_DIR/aoc-mission-control-native"
    fi
  else
    log "WARNING: Failed to build aoc-mission-control."
  fi
else
  log "WARNING: cargo not found. Skipping Rust builds. You must install aoc-cli manually."
fi

# 3. Dependencies
log "Checking dependencies..."

missing_required=()
missing_optional=()

if ! have curl && ! have wget; then
  if ! ensure_tool curl "curl"; then
    if ! ensure_tool wget "wget"; then
      missing_required+=("curl/wget")
    fi
  fi
fi

if ! ensure_tool zellij "zellij"; then
  missing_required+=("zellij")
fi
if ! ensure_tool yazi "yazi"; then
  missing_required+=("yazi")
fi
if ! ensure_tool fzf "fzf"; then
  missing_required+=("fzf")
fi

if ! ensure_tool tmux "tmux"; then
  missing_optional+=("tmux")
fi
if ! ensure_tool chafa "chafa"; then
  missing_optional+=("chafa")
fi
if ! ensure_tool ffmpeg "ffmpeg"; then
  missing_optional+=("ffmpeg")
fi
if ! ensure_tool pdftoppm "poppler-utils (pdftoppm)"; then
  missing_optional+=("poppler-utils")
fi
if ! ensure_tool rsvg-convert "librsvg (rsvg-convert)"; then
  missing_optional+=("librsvg")
fi
if ! ensure_tool rg "ripgrep (rg)"; then
  missing_optional+=("ripgrep")
fi
if ! ensure_bat; then
  missing_optional+=("bat")
fi

if ((${#missing_required[@]} > 0)); then
  warn "Missing required tools: ${missing_required[*]}"
  warn "Cannot continue without required tools. Install them and re-run install.sh."
  exit 1
fi
if ((${#missing_optional[@]} > 0)); then
  warn "Missing optional tools: ${missing_optional[*]}"
fi

# Micro
if ! command -v micro >/dev/null 2>&1; then
  if [[ ! -f "$BIN_DIR/micro" ]]; then
    log "Downloading micro..."
    if have curl; then
      if ! curl -fsSL https://getmic.ro | bash; then
        warn "Failed to download micro via curl."
      fi
    elif have wget; then
      if ! wget -qO- https://getmic.ro | bash; then
        warn "Failed to download micro via wget."
      fi
    else
      warn "curl or wget required to download micro."
    fi
    if [[ -f "micro" ]]; then
      mv micro "$BIN_DIR/micro"
    fi
  fi
else
  log "Micro found."
fi

# ZJStatus
ZJSTATUS_PATH="$HOME/.config/zellij/plugins/zjstatus.wasm"
if [[ ! -f "$ZJSTATUS_PATH" ]]; then
  log "Downloading zjstatus.wasm..."
  if have curl; then
    if ! curl -fsSL -o "$ZJSTATUS_PATH" https://github.com/dj95/zjstatus/releases/latest/download/zjstatus.wasm; then
      warn "Failed to download zjstatus.wasm via curl."
    fi
  elif have wget; then
    if ! wget -qO "$ZJSTATUS_PATH" https://github.com/dj95/zjstatus/releases/latest/download/zjstatus.wasm; then
      warn "Failed to download zjstatus.wasm via wget."
    fi
  else
    warn "curl or wget required to download zjstatus.wasm."
  fi
fi

# 4. Generate & Install Configs
log "Generating configurations..."

# Zellij Layout
# Replace placeholders in template
PROJECTS_BASE="$HOME/dev"
[[ ! -d "$PROJECTS_BASE" ]] && PROJECTS_BASE="$HOME"

sed \
  -e "s|{{HOME}}|$HOME|g" \
  -e "s|{{PROJECTS_BASE}}|$PROJECTS_BASE|g" \
  "$ROOT_DIR/zellij/layouts/aoc.kdl.template" > "$HOME/.config/zellij/layouts/aoc.kdl"

sed \
  -e "s|{{HOME}}|$HOME|g" \
  "$ROOT_DIR/zellij/layouts/minimal.kdl.template" > "$HOME/.config/zellij/layouts/minimal.kdl"

log "Generated layouts in $HOME/.config/zellij/layouts/"

sed \
  -e "s|{{HOME}}|$HOME|g" \
  "$ROOT_DIR/zellij/aoc.config.kdl.template" > "$HOME/.config/zellij/aoc.config.kdl"

# Copy other configs
install -m 0644 "$ROOT_DIR/yazi/yazi.toml" "$HOME/.config/yazi/yazi.toml"
install -m 0755 "$ROOT_DIR/yazi/preview.sh" "$HOME/.config/yazi/preview.sh"
install -m 0644 "$ROOT_DIR/yazi/keymap.toml" "$HOME/.config/yazi/keymap.toml"
install -m 0644 "$ROOT_DIR/yazi/theme.toml" "$HOME/.config/yazi/theme.toml"
install -m 0644 "$ROOT_DIR/yazi/init.lua" "$HOME/.config/yazi/init.lua"
install -m 0644 "$ROOT_DIR/micro/bindings.json" "${MICRO_CONFIG_HOME:-${XDG_CONFIG_HOME:-$HOME/.config}/micro}/bindings.json"
install -m 0644 "$ROOT_DIR/config/codex-tmux.conf" "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/codex-tmux.conf"
install -m 0644 "$ROOT_DIR/config/btop.conf" "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/btop/btop.conf"

# AOC default skills
if [[ -d "$ROOT_DIR/.aoc/skills" ]]; then
  for d in "$ROOT_DIR/.aoc/skills"/*; do
    [[ -d "$d" ]] || continue
    dest="${XDG_CONFIG_HOME:-$HOME/.config}/aoc/skills/$(basename "$d")"
    if [[ -e "$dest" ]]; then
      continue
    fi
    cp -R "$d" "$dest"
  done
  if [[ -f "$ROOT_DIR/.aoc/skills/manifest.toml" && ! -f "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/skills/manifest.toml" ]]; then
    cp "$ROOT_DIR/.aoc/skills/manifest.toml" "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/skills/manifest.toml"
  fi
fi

# Upstream Taskmaster PRD templates for project seeding
if [[ -d "$ROOT_DIR/.taskmaster/templates" ]]; then
  for f in "$ROOT_DIR/.taskmaster/templates"/example_prd*.txt; do
    [[ -f "$f" ]] || continue
    dest="${XDG_CONFIG_HOME:-$HOME/.config}/aoc/taskmaster/templates/$(basename "$f")"
    if [[ -f "$dest" ]]; then
      continue
    fi
    cp "$f" "$dest"
  done
fi

# AOC default PI prompt templates
if [[ -d "$ROOT_DIR/.aoc/prompts/pi" ]]; then
  for f in "$ROOT_DIR/.aoc/prompts/pi"/*.md; do
    [[ -f "$f" ]] || continue
    dest="${XDG_CONFIG_HOME:-$HOME/.config}/aoc/prompts/pi/$(basename "$f")"
    if [[ ! -f "$dest" ]]; then
      cp "$f" "$dest"
    fi
  done
fi

# AOC default PI extension templates
if [[ -d "$ROOT_DIR/.pi/extensions" ]]; then
  mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/pi/extensions"
  for f in "$ROOT_DIR/.pi/extensions"/*.ts; do
    [[ -f "$f" ]] || continue
    dest="${XDG_CONFIG_HOME:-$HOME/.config}/aoc/pi/extensions/$(basename "$f")"
    if [[ ! -f "$dest" ]]; then
      cp "$f" "$dest"
      continue
    fi

    if ! cmp -s "$f" "$dest"; then
      cp "$f" "$dest"
    fi
  done
fi

# Optional skills and PI prompts (MoreMotion)
if [[ -d "$ROOT_DIR/.aoc/skills-optional" ]]; then
  for d in "$ROOT_DIR/.aoc/skills-optional"/*; do
    [[ -d "$d" ]] || continue
    dest="${XDG_CONFIG_HOME:-$HOME/.config}/aoc/skills-optional/$(basename "$d")"
    if [[ -e "$dest" ]]; then
      continue
    fi
    cp -R "$d" "$dest"
  done
fi
if [[ -d "$ROOT_DIR/.aoc/prompts-optional/pi" ]]; then
  for f in "$ROOT_DIR/.aoc/prompts-optional/pi"/*.md; do
    [[ -f "$f" ]] || continue
    dest="${XDG_CONFIG_HOME:-$HOME/.config}/aoc/prompts-optional/pi/$(basename "$f")"
    if [[ -f "$dest" ]]; then
      continue
    fi
    cp "$f" "$dest"
  done
fi

# Yazi Plugins
if [[ -d "$ROOT_DIR/yazi/plugins" ]]; then
  shopt -s nullglob
  for d in "$ROOT_DIR/yazi/plugins/"*.yazi; do
    [[ -d "$d" ]] || continue
    dest="$HOME/.config/yazi/plugins/$(basename "$d")"
    mkdir -p "$dest"
    for f in "$d"/*.lua; do
      [[ -f "$f" ]] || continue
      install -m 0644 "$f" "$dest/$(basename "$f")"
    done
  done
  shopt -u nullglob
fi

# Bash integration for dynamic layout shortcuts (aoc.<layout>)
bashrc_file="$HOME/.bashrc"
bashrc_block_start="# >>> AOC bash integration >>>"

if [[ ! -f "$bashrc_file" ]]; then
  : > "$bashrc_file"
fi

if ! grep -Fq "$bashrc_block_start" "$bashrc_file"; then
  cat <<'EOF' >> "$bashrc_file"

# >>> AOC bash integration >>>
if command -v aoc-layout >/dev/null 2>&1; then
  eval "$(aoc-layout --shell-init bash)"
fi
# <<< AOC bash integration <<<
EOF
  log "Enabled Bash layout shortcuts in $bashrc_file"
fi

if ! install_pi_agent_if_enabled; then
  if is_truthy "${AOC_INSTALL_PI_REQUIRED:-1}"; then
    warn "PI agent installation failed and is required (set AOC_INSTALL_PI_REQUIRED=0 to continue anyway)."
    exit 1
  fi
  warn "PI agent installation failed; continuing because AOC_INSTALL_PI_REQUIRED=0."
fi

if is_truthy "${AOC_INSTALL_AUTO_INIT:-1}"; then
  init_target="${AOC_INIT_TARGET:-$PWD}"
  if [[ -d "$init_target" ]]; then
    init_bin=""
    if [[ -x "$BIN_DIR/aoc-init" ]]; then
      init_bin="$BIN_DIR/aoc-init"
    elif have aoc-init; then
      init_bin="$(command -v aoc-init)"
    elif [[ -x "$ROOT_DIR/bin/aoc-init" ]]; then
      init_bin="$ROOT_DIR/bin/aoc-init"
    fi

    if [[ -n "$init_bin" ]]; then
      log "Running aoc-init in $init_target..."
      if "$init_bin" "$init_target"; then
        log "aoc-init completed (RTK config seeded in $init_target/.aoc/rtk.toml)."
      else
        warn "aoc-init failed for $init_target. Run 'aoc-init $init_target' manually."
      fi
    else
      warn "aoc-init binary not found after install; run it manually in your project."
    fi
  else
    warn "AOC_INIT_TARGET does not exist: $init_target"
  fi
else
  log "Skipping automatic aoc-init (set AOC_INSTALL_AUTO_INIT=1 to enable)."
fi

install_omo_if_enabled

log "AOC Installed Successfully!"
log "Run 'aoc' to start."

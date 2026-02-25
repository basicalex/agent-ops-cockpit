#!/usr/bin/env bash
set -euo pipefail

AOC_REPO="${AOC_REPO:-}"
AOC_REF="${AOC_REF:-}"
AOC_YES="${AOC_YES:-0}"
AOC_SKIP_DOCTOR="${AOC_SKIP_DOCTOR:-0}"

usage() {
  cat <<'EOF'
Usage: bootstrap.sh [options]

Options:
  --repo <owner/name>   GitHub repo (required unless auto-detected)
  --ref <tag-or-branch> Release tag or branch to install
  --yes                 Non-interactive install
  --skip-doctor         Skip post-install aoc-doctor check
  -h, --help            Show help

Environment:
  AOC_REPO, AOC_REF, AOC_YES, AOC_SKIP_DOCTOR
EOF
}

log() { printf '>> %s\n' "$*"; }
warn() { printf '!! %s\n' "$*" >&2; }
have() { command -v "$1" >/dev/null 2>&1; }

parse_repo_from_url() {
  local url="$1"
  case "$url" in
    git@github.com:*)
      url="${url#git@github.com:}"
      ;;
    https://github.com/*)
      url="${url#https://github.com/}"
      ;;
    http://github.com/*)
      url="${url#http://github.com/}"
      ;;
    *)
      return 1
      ;;
  esac

  url="${url%.git}"
  if [[ "$url" == */* ]]; then
    printf '%s\n' "$url"
    return 0
  fi

  return 1
}

resolve_repo() {
  local detected=""

  if [[ -n "$AOC_REPO" ]]; then
    printf '%s\n' "$AOC_REPO"
    return 0
  fi

  if [[ -n "${GITHUB_REPOSITORY:-}" ]]; then
    printf '%s\n' "$GITHUB_REPOSITORY"
    return 0
  fi

  if have git; then
    detected="$(git config --get remote.origin.url 2>/dev/null || true)"
    if [[ -n "$detected" ]] && detected="$(parse_repo_from_url "$detected" 2>/dev/null)"; then
      printf '%s\n' "$detected"
      return 0
    fi
  fi

  warn "Could not determine GitHub repo automatically."
  warn "Re-run with: --repo <owner/name>"
  return 1
}

fetch_stdout() {
  local url="$1"
  if have curl; then
    curl -fsSL "$url"
  elif have wget; then
    wget -qO- "$url"
  else
    return 1
  fi
}

download_file() {
  local url="$1"
  local out="$2"
  if have curl; then
    curl -fsSL -o "$out" "$url"
  elif have wget; then
    wget -qO "$out" "$url"
  else
    return 1
  fi
}

read_checksum() {
  local checksum_file="$1"
  local expected=""

  if [[ ! -s "$checksum_file" ]]; then
    return 1
  fi

  read -r expected _ < "$checksum_file"
  if [[ ! "$expected" =~ ^[A-Fa-f0-9]{64}$ ]]; then
    return 1
  fi

  printf '%s\n' "$expected"
}

compute_sha256() {
  local file="$1"
  if have sha256sum; then
    sha256sum "$file" | awk '{print $1}'
  elif have shasum; then
    shasum -a 256 "$file" | awk '{print $1}'
  elif have openssl; then
    openssl dgst -sha256 "$file" | awk '{print $NF}'
  else
    return 1
  fi
}

verify_checksum() {
  local file="$1"
  local checksum_file="$2"
  local expected actual

  if ! expected="$(read_checksum "$checksum_file")"; then
    warn "Could not parse checksum file: $checksum_file"
    return 1
  fi

  if ! actual="$(compute_sha256 "$file")"; then
    warn "No SHA-256 tool available (sha256sum, shasum, or openssl)."
    return 1
  fi

  if [[ "$actual" != "$expected" ]]; then
    warn "Checksum verification failed for $(basename "$file")."
    return 1
  fi

  return 0
}

resolve_ref() {
  local repo="$1"
  if [[ -n "$AOC_REF" ]]; then
    printf '%s\n' "$AOC_REF"
    return 0
  fi

  local json tag
  if ! json="$(fetch_stdout "https://api.github.com/repos/${repo}/releases/latest" 2>/dev/null)"; then
    printf 'main\n'
    return 0
  fi

  tag="$(printf '%s\n' "$json" | awk -F'"' '/"tag_name"[[:space:]]*:/ {print $4; exit}')"
  if [[ -n "$tag" ]]; then
    printf '%s\n' "$tag"
  else
    printf 'main\n'
  fi
}

detect_target() {
  local os arch
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"
  case "${os}/${arch}" in
    linux/x86_64) printf 'x86_64-unknown-linux-musl\n' ;;
    linux/aarch64|linux/arm64) printf 'aarch64-unknown-linux-musl\n' ;;
    darwin/x86_64) printf 'x86_64-apple-darwin\n' ;;
    darwin/aarch64|darwin/arm64) printf 'aarch64-apple-darwin\n' ;;
    *) printf '\n' ;;
  esac
}

confirm_if_needed() {
  if [[ "$AOC_YES" == "1" ]]; then
    return 0
  fi

  printf 'Install AOC to user-local paths (~/.local/bin, ~/.config)? [Y/n]: '
  read -r ans
  case "${ans:-Y}" in
    n|N|no|NO)
      log "Install cancelled."
      return 1
      ;;
    *)
      return 0
      ;;
  esac
}

run_fallback_source_install() {
  local repo="$1"
  local ref="$2"
  local workdir="$3"
  local archive="$workdir/aoc-src.tar.gz"
  local tag_url="https://github.com/${repo}/archive/refs/tags/${ref}.tar.gz"
  local head_url="https://github.com/${repo}/archive/refs/heads/${ref}.tar.gz"
  local plain_url="https://github.com/${repo}/archive/${ref}.tar.gz"
  local src_root=""
  local candidate

  if ! download_file "$tag_url" "$archive"; then
    warn "Tag archive not found for '$ref'; trying branch archive."
    if ! download_file "$head_url" "$archive"; then
      warn "Branch archive not found for '$ref'; trying plain ref archive (e.g. SHA)."
      download_file "$plain_url" "$archive"
    fi
  fi

  tar -xzf "$archive" -C "$workdir"
  for candidate in "$workdir"/*; do
    [[ -d "$candidate" ]] || continue
    src_root="$candidate"
    break
  done

  if [[ -z "$src_root" ]]; then
    warn "Could not locate extracted source directory."
    return 1
  fi

  if [[ ! -f "$src_root/install.sh" ]]; then
    warn "install.sh not found in downloaded archive."
    return 1
  fi

  if ! confirm_if_needed; then
    return 0
  fi

  bash "$src_root/install.sh"

  if [[ "$AOC_SKIP_DOCTOR" != "1" ]] && have aoc-doctor; then
    log "Running aoc-doctor..."
    if ! aoc-doctor; then
      warn "aoc-doctor reported issues."
    fi
  fi
}

main() {
  while (($# > 0)); do
    case "$1" in
      --repo)
        if (($# < 2)); then
          warn "--repo requires a value."
          exit 1
        fi
        AOC_REPO="$2"
        shift 2
        ;;
      --ref)
        if (($# < 2)); then
          warn "--ref requires a value."
          exit 1
        fi
        AOC_REF="$2"
        shift 2
        ;;
      --yes)
        AOC_YES=1
        shift
        ;;
      --skip-doctor)
        AOC_SKIP_DOCTOR=1
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        warn "Unknown option: $1"
        usage
        exit 1
        ;;
    esac
  done

  if ! have tar; then
    warn "tar is required to install AOC."
    exit 1
  fi

  if ! have curl && ! have wget; then
    warn "curl or wget is required to download AOC."
    exit 1
  fi

  local repo ref target workdir installer_archive installer_checksum installer_url installer_checksum_url installer_bin
  repo="$(resolve_repo)"
  ref="$(resolve_ref "$repo")"
  target="$(detect_target)"
  workdir="$(mktemp -d "${TMPDIR:-/tmp}/aoc-bootstrap.XXXXXX")"
  trap "rm -rf '$workdir'" EXIT

  if [[ -n "$target" ]]; then
    local installer_args=()
    installer_archive="$workdir/aoc-installer-${target}.tar.gz"
    installer_checksum="$workdir/aoc-installer-${target}.tar.gz.sha256"
    installer_url="https://github.com/${repo}/releases/download/${ref}/aoc-installer-${target}.tar.gz"
    installer_checksum_url="${installer_url}.sha256"
    installer_args=(--repo "$repo" --ref "$ref")
    if [[ "$AOC_YES" == "1" ]]; then
      installer_args+=(--yes)
    fi
    if [[ "$AOC_SKIP_DOCTOR" == "1" ]]; then
      installer_args+=(--skip-doctor)
    fi

    log "Trying portable installer binary (${target}) from release ${ref}..."
    if download_file "$installer_url" "$installer_archive" && download_file "$installer_checksum_url" "$installer_checksum"; then
      if verify_checksum "$installer_archive" "$installer_checksum"; then
        tar -xzf "$installer_archive" -C "$workdir"
        installer_bin="$workdir/aoc-installer"
        if [[ -x "$installer_bin" ]]; then
          exec "$installer_bin" "${installer_args[@]}"
        fi
        warn "Portable installer asset unpacked but executable was not found."
      else
        warn "Portable installer checksum could not be verified; using source installer fallback."
      fi
    else
      warn "Portable installer asset unavailable for ${target}; using source installer fallback."
    fi
  else
    warn "No portable installer target for this platform; using source installer fallback."
  fi

  log "Running source installer fallback from GitHub archive..."
  run_fallback_source_install "$repo" "$ref" "$workdir"
}

main "$@"

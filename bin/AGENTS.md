# Repository Guidelines

Scope: `bin`

## Local Contracts
- Preserve `bin/*` as public PATH entrypoints: Bash wrappers keep shebangs, `set -euo pipefail`, `exec` handoff on delegation, and existing colocated/repo-relative command resolution before PATH fallback.
- Do not hand-edit generated or managed outputs/cache under `bin`; update the source/template/regeneration path instead, and never treat `bin/__pycache__/*.pyc` as source.
- For Python CLIs in `bin`, keep argument parsing and runtime work behind `main()` and `if __name__ == "__main__"`; imports should not start services, clean processes, mutate files, or perform network work.

## Verification
- `PYTHONDONTWRITEBYTECODE=1 python3 -B bin/<changed-python-cli> --help >/dev/null`
- `bash -n bin/<changed-bash-wrapper>`
- `bash -n bin/aoc-context bin/aoc-hyperframes bin/aoc-init`
- `bin/<changed-command> --help >/dev/null`

## Do Not
- Do not create a local AGENTS file inside `bin/__pycache__`, review `.pyc` files as source, or patch generated/managed files without identifying the source generator or asset marker.
- Do not perform service startup, process cleanup, filesystem mutation, or network work at import time; do not hide CLI behavior outside parser/main entrypoints.
- Do not replace `exec` dispatch with plain subprocess calls, remove strict shell mode from Bash wrappers, simplify existing resolution to PATH-only lookup, or remove local/debug/release fallbacks without preserving the development workflow.

## Update When
- Update when adding or changing Python CLI files in `bin`, parser setup, or importable helper modules.
- Update when adding or changing a public command in `bin`, wrapper delegation, local binary lookup order, or help/startup behavior.
- Update when generated/managed markers move, regeneration ownership changes, or cache/source boundaries under `bin` change.

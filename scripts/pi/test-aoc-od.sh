#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

bash -n bin/aoc-od
bin/aoc-od --help >/dev/null
bin/aoc-od doctor >/dev/null

project="$(mktemp -d)"
trap 'rm -rf "$project"' EXIT
mkdir -p "$project/.aoc" "$project/.od/artifacts/2026-demo"
cat >"$project/.od/artifacts/2026-demo/index.html" <<'HTML'
<!doctype html><html><body>OD demo</body></html>
HTML

bin/aoc-od --root "$project" link >/dev/null
[[ -f "$project/.aoc/open-design/link.json" ]]

bin/aoc-od --root "$project" import latest >/dev/null
[[ -f "$project/design-artifacts/od/2026-demo/index.html" ]]
[[ -f "$project/design-artifacts/od/2026-demo/aoc-open-design-artifact.json" ]]
[[ -f "$project/.aoc/open-design/artifacts.json" ]]

python3 -m json.tool "$project/.aoc/open-design/link.json" >/dev/null
python3 -m json.tool "$project/.aoc/open-design/artifacts.json" >/dev/null
python3 -m json.tool "$project/design-artifacts/od/2026-demo/aoc-open-design-artifact.json" >/dev/null

python3 - "$project" <<'PY'
import json, sys
from pathlib import Path
root = Path(sys.argv[1])
link = json.loads((root / '.aoc/open-design/link.json').read_text())
artifacts = json.loads((root / '.aoc/open-design/artifacts.json').read_text())
assert link['aocImportDir'] == 'design-artifacts/od'
assert link['odArtifactsDir'] == '.od/artifacts'
assert artifacts['artifacts'][-1]['importedPath'] == 'design-artifacts/od/2026-demo'
PY

echo "OK: aoc-od link/import smoke passed"

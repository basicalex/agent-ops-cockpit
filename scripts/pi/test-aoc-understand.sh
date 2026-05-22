#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

bash -n bin/aoc-understand
bin/aoc-understand --help >/dev/null
bin/aoc-understand status >/dev/null
bin/aoc-understand doctor >/dev/null

project="$(mktemp -d)"
trap 'rm -rf "$project"' EXIT
mkdir -p "$project/.aoc" "$project/.understand-anything"
cat > "$project/.understand-anything/knowledge-graph.json" <<'JSON'
{
  "version": "1.0.0",
  "project": {
    "name": "fixture-project",
    "languages": ["Rust", "Shell"],
    "frameworks": [],
    "description": "fixture",
    "analyzedAt": "2026-05-22T00:00:00Z",
    "gitCommitHash": "fixture"
  },
  "nodes": [
    {"id":"file:bin/aoc","type":"file","name":"aoc","filePath":"bin/aoc","summary":"entrypoint","tags":["cli"],"complexity":"simple"}
  ],
  "edges": [],
  "layers": [
    {"id":"cli","name":"CLI","description":"Command entrypoints","nodeIds":["file:bin/aoc"]}
  ],
  "tour": [
    {"order":1,"title":"Start here","description":"Open the CLI entrypoint","nodeIds":["file:bin/aoc"]}
  ]
}
JSON

bin/aoc-understand --root "$project" map-sync >/dev/null
[[ -f "$project/.aoc/map/pages/understand-overview.html" ]]
[[ -f "$project/.aoc/map/diagrams/understand-overview.mmd" ]]
grep -q "fixture-project" "$project/.aoc/map/pages/understand-overview.html"

mkdir -p "$project/.pi/skills"
cp -a .pi/skills/aoc-understand "$project/.pi/skills/"
python3 - <<'PY' "$project/.pi/skills/aoc-understand/SKILL.md"
import pathlib, sys
p = pathlib.Path(sys.argv[1])
text = p.read_text()
assert 'name: aoc-understand' in text
assert 'teach' in text.lower()
PY

echo "OK: aoc-understand smoke passed"

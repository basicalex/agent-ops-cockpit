#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

project="$tmp/project"
omp_runtime="$tmp/omp-agent"
omp_config="$omp_runtime/config.yml"
mkdir -p "$project" "$omp_runtime"

AOC_INIT_SKIP_BUILD=1 AOC_OMP_PROFILES=core,hyperframes AOC_OMP_AGENT_DIR="$omp_runtime" AOC_OMP_AGENT_CONFIG="$omp_config" bash "$root/bin/aoc-init" "$project"

for required in \
  extensions/aoc-codegraph.ts \
  extensions/aoc-mind.ts \
  extensions/aoc-dox.ts \
  extensions/aoc-style.ts \
  extensions/aoc-profile.ts \
  extensions/aoc-brand-content.ts \
  agents/brand-strategy.md \
  agents/brand-concept.md \
  agents/svg-asset.md \
  agents/hyperframes-content.md \
  skills/aoc-hyperframes/SKILL.md \
  skills/hyperframes/SKILL.md \
  skills/hyperframes-cli/SKILL.md \
  skills/website-to-hyperframes/SKILL.md \
  skills/gsap/SKILL.md; do
  if [[ ! -f "$omp_runtime/$required" ]]; then
    echo "ERROR: missing synced OMP runtime asset: $required" >&2
    exit 1
  fi
done

bash "$root/bin/aoc-hyperframes" --root "$project" brand init --brand demo-brand
bash "$root/bin/aoc-hyperframes" --root "$project" bootstrap-asset-system
bash "$root/bin/aoc-hyperframes" --root "$project" brand check --no-lint
bash "$root/bin/aoc-hyperframes" --root "$project" brand board --write
bash "$root/bin/aoc-hyperframes" --root "$project" brand campaign launch --audience founders --channels meta,reel --durations 15s,6s --concept signal
bash "$root/bin/aoc-hyperframes" --root "$project" brand check --no-lint
bash "$root/bin/aoc-hyperframes" --root "$project" check --no-lint

bash "$root/bin/aoc-html-video" --root "$project" status
bash "$root/bin/aoc-html-video" --root "$project" project create --from hyperframes/docs/content-campaign-plan.md --id launch
bash "$root/bin/aoc-hyperframes" --root "$project" brand export
bash "$root/bin/aoc-hyperframes" --root "$project" brand export --output hyperframes/generated/brand-content/manifest-second.json

test -f "$project/hyperframes/docs/brand-strategy.md"
test -f "$project/hyperframes/docs/campaign-review-board.md"
test -f "$project/hyperframes/docs/content-campaign-plan.md"
test -f "$project/hyperframes/docs/svg-asset-manifest.md"
test -f "$project/hyperframes/compositions/_playgrounds/brand-campaign-board.html"
test -f "$project/hyperframes/compositions/ads/founders/meta-15s-signal-v1.html"
test -f "$project/hyperframes/compositions/social/founders/reel-6s-signal-v1.html"
test -f "$project/hyperframes/generated/html-video/launch/project.json"
test -f "$project/hyperframes/generated/brand-content/manifest.json"
test -f "$project/hyperframes/generated/brand-content/manifest-second.json"

python3 - "$root" <<'PY'
from pathlib import Path
import sys
root = Path(sys.argv[1])
for path in [root / '.omp/extensions/aoc-brand-content.ts']:
    text = path.read_text(encoding='utf-8')
    assert 'brand-content' in text
    assert 'hyperframes-director' in text
for path in (root / '.omp/agents').glob('*.md'):
    if path.name not in {'brand-strategy.md', 'brand-concept.md', 'svg-asset.md', 'hyperframes-content.md'}:
        continue
    text = path.read_text(encoding='utf-8')
    for required in ('name:', 'description:', 'tools:', 'spawns:', 'model: openai-codex/gpt-5.5', 'thinking-level: high'):
        assert required in text, f'{path}: missing {required}'
PY

python3 - "$project" <<'PY'
from pathlib import Path
import json
import sys

project = Path(sys.argv[1])
first = json.loads((project / 'hyperframes/generated/brand-content/manifest.json').read_text(encoding='utf-8'))
second = json.loads((project / 'hyperframes/generated/brand-content/manifest-second.json').read_text(encoding='utf-8'))
assert first['schema'] == 'aoc.brand_content.bundle.v1'
assert first['campaigns'][0]['id'] == 'launch'
assert first['campaigns'][0]['htmlVideoManifestPath'] == 'hyperframes/generated/html-video/launch/project.json'
assert any(artifact['type'] == 'storyboard' and artifact['engine'] == 'html_video' for artifact in first['artifacts'])
assert first['contentGraphs'] and first['contentGraphs'][0]['nodeCount'] >= 0
first_ids = sorted(artifact['id'] for artifact in first['artifacts'])
second_ids = sorted(artifact['id'] for artifact in second['artifacts'])
assert first_ids == second_ids, 'artifact IDs must remain stable across repeated exports'
assert len(first_ids) == len(set(first_ids)), 'artifact IDs must be unique'
PY

printf 'AOC branded content OMP smoke passed: %s\n' "$project"

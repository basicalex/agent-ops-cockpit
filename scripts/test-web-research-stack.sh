#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 -m py_compile bin/aoc-search bin/aoc-fetch bin/aoc-render

fetch_json="$(bin/aoc-fetch https://example.com --format json --max-chars 500)"
FETCH_JSON="$fetch_json" python3 - <<'PY'
import json, os
payload=json.loads(os.environ['FETCH_JSON'])
assert payload['status'] == 200
assert 'Example Domain' in payload.get('title','')
assert payload.get('text')
PY

package_json="$(bin/aoc-search query --mode package --direct --json --limit 1 'requests')"
PACKAGE_JSON="$package_json" python3 - <<'PY'
import json, os
payload=json.loads(os.environ['PACKAGE_JSON'])
assert payload['provider'] == 'package-registries'
assert payload['results']
PY

search_json="$(bin/aoc-search query --mode github --json --limit 1 'h4ckf0r0day/obscura')"
SEARCH_JSON="$search_json" python3 - <<'PY'
import json, os
payload=json.loads(os.environ['SEARCH_JSON'])
assert payload['provider'] == 'github'
assert payload['results'] and payload['results'][0]['url'].endswith('/h4ckf0r0day/obscura')
PY

set +e
render_status="$(bin/aoc-render status --json)"
render_code=$?
set -e
RENDER_STATUS="$render_status" RENDER_CODE="$render_code" python3 - <<'PY'
import json, os
payload=json.loads(os.environ['RENDER_STATUS'])
assert payload['backend'] == 'obscura'
assert payload['serverRequired'] is False
assert os.environ['RENDER_CODE'] in {'0', '3'}
PY

if [ "$render_code" = "0" ]; then
  render_json="$(bin/aoc-render https://example.com --format json --max-chars 500)"
  RENDER_JSON="$render_json" python3 - <<'PY'
import json, os
payload=json.loads(os.environ['RENDER_JSON'])
assert payload['backend'] == 'obscura'
assert 'Example Domain' in payload['content']
PY
fi

echo "web research stack smoke passed"

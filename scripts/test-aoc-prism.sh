#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d)"
server_pid=""
cleanup() {
    if [[ -n "$server_pid" ]]; then
        kill "$server_pid" 2>/dev/null || true
        wait "$server_pid" 2>/dev/null || true
    fi
    rm -rf "$tmp_dir"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

python3 -u - "$tmp_dir" <<'PY' &
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import sys
import threading
import time

root = Path(sys.argv[1])
lock = threading.Lock()


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        record = {'method': self.command, 'path': self.path,
                  'headers': dict(self.headers), 'body': body}
        with lock, (root / 'requests.jsonl').open('a') as stream:
            stream.write(json.dumps(record) + '\n')
        mode = json.loads((root / 'mode.json').read_text()) if (root / 'mode.json').exists() else {}
        time.sleep(mode.get('delay', 0))
        if mode.get('disconnect'):
            self.close_connection = True
            return
        response = json.dumps(mode.get('body', {'accepted': True})).encode()
        self.send_response(mode.get('code', 200))
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(response) + mode.get('extra_length', 0)))
        if mode.get('redirect'):
            self.send_header('Location', mode['redirect'])
        self.end_headers()
        try:
            self.wfile.write(response)
        except (BrokenPipeError, ConnectionResetError):
            pass


server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
(root / 'port').write_text(str(server.server_port))
server.serve_forever()
PY
server_pid=$!

python3 - "$root/bin/aoc-prism" "$tmp_dir" <<'PY'
from datetime import datetime
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import time

cli = Path(sys.argv[1])
tmp = Path(sys.argv[2])
deadline = time.monotonic() + 5
while not (tmp / 'port').exists():
    assert time.monotonic() < deadline, 'stub did not start'
    time.sleep(0.02)
base = 'http://127.0.0.1:' + (tmp / 'port').read_text()
token = 'aoc-test-token-not-real'
env = {key: value for key, value in os.environ.items()
       if not key.startswith('AOC_PRISM') and key.lower() not in
       ('http_proxy', 'https_proxy', 'all_proxy', 'no_proxy')}
env.update({'HOME': str(tmp / 'home'), 'XDG_STATE_HOME': str(tmp / 'state'),
            'AOC_PRISM_URL': base + '/', 'AOC_PRISM_TOKEN': token,
            'AOC_PRISM_TOKEN_FILE': str(tmp / 'absent-token.json'),
            'AOC_PRISM_TIMEOUT': '1', 'PYTHONDONTWRITEBYTECODE': '1', 'NO_PROXY': '*'})
log = tmp / 'state/aoc/prism/notify.log'
checks = 0


def requests():
    path = tmp / 'requests.jsonl'
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def mode(**settings):
    (tmp / 'mode.json').write_text(json.dumps(settings))


def invoke(*args, overrides=None, code=0, sent=1, logged=True):
    global checks
    before_requests = len(requests())
    before_logs = len(log.read_text().splitlines()) if log.exists() else 0
    call_env = {**env, **(overrides or {})}
    call_env = {key: value for key, value in call_env.items() if value is not None}
    result = subprocess.run([str(cli), *args], env=call_env, text=True,
                            capture_output=True, timeout=5, check=False)
    assert result.returncode == code, (args, result.returncode, result.stderr)
    assert token not in result.stdout + result.stderr, 'token leaked to process output'
    assert len(requests()) == before_requests + sent, (args, 'wrong request count')
    lines = log.read_text().splitlines() if log.exists() else []
    assert len(lines) == before_logs + int(logged), (args, 'wrong log count')
    assert token not in '\n'.join(lines), 'token leaked to log'
    if logged:
        stamp, command, status, http_code, preview = lines[-1].split(' ', 4)
        assert datetime.fromisoformat(stamp).tzinfo is not None
        assert command == args[0]
        assert len(json.loads(preview)) <= 120
    if code in (0, 1):
        assert not result.stderr, result.stderr
        if '--json' not in args and args[0] != 'status':
            assert not result.stdout, result.stdout
    checks += 1
    return result


activity = ['activity', '--agent', ' aoc-master@project ', '--status', 'started', '--summary', ' Run ready ']
result = invoke(*activity, '--repo', 'owner/project', '--issue-url', 'https://github.com/owner/project/issues/7',
                '--task-ref', 'owner/project#7', '--workspace-id', 'work-1', '--json')
assert json.loads(result.stdout) == {'accepted': True}
record = requests()[-1]
assert record['method'] == 'POST' and record['path'] == '/api/v1/agent-activity'
assert record['body'] == {'agent': 'aoc-master@project', 'status': 'started', 'summary': 'Run ready',
                          'repo': 'owner/project', 'issueUrl': 'https://github.com/owner/project/issues/7',
                          'taskRef': 'owner/project#7', 'workspaceId': 'work-1'}
headers = {key.lower(): value for key, value in record['headers'].items()}
assert headers['authorization'] == 'Bearer ' + token
assert headers['content-type'] == 'application/json' and headers['user-agent'] == 'aoc-prism/1'
invoke(*activity)
assert requests()[-1]['body'] == {'agent': 'aoc-master@project', 'status': 'started', 'summary': 'Run ready'}
for status in ('progress', 'blocked', 'done', 'failed'):
    invoke('activity', '--agent', 'worker', '--status', status, '--summary', status)
    assert requests()[-1]['body']['status'] == status

notify = ['notify', '--title', ' Attention ', '--body', ' Choose next step ']
invoke(*notify)
assert requests()[-1]['path'] == '/api/v1/notify'
assert requests()[-1]['body'] == {'source': 'aoc', 'title': 'Attention', 'body': 'Choose next step'}
invoke(*notify, '--url', 'http://example.test/item', '--severity', 'urgent', '--kind', 'decision',
       '--dedupe-key', 'item:7', '--issue-url', 'https://example.test/issue', '--workspace-id', 'work-2')
assert requests()[-1]['body'] == {'source': 'aoc', 'title': 'Attention', 'body': 'Choose next step',
                                 'url': 'http://example.test/item', 'severity': 'urgent', 'kind': 'decision',
                                 'dedupeKey': 'item:7', 'issueUrl': 'https://example.test/issue', 'workspaceId': 'work-2'}
invoke('resolve', '--dedupe-key', ' item:7 ', '--json')
assert requests()[-1]['path'] == '/api/v1/notify/resolve'
assert requests()[-1]['body'] == {'dedupeKey': 'item:7'}

mode(code=500, body={'error': token})
result = invoke(*activity, '--json')
assert not result.stdout
assert ' activity HTTPError 500 ' in log.read_text().splitlines()[-1]
result = invoke(*activity, '--strict', '--json', code=1)
assert not result.stdout
mode(code=401)
invoke(*notify, '--strict', code=1)
mode(code=302, redirect=base + '/must-not-follow')
invoke(*activity, '--strict', code=1)
assert ' HTTPError 302 ' in log.read_text().splitlines()[-1]
mode(delay=0.6)
start = time.monotonic()
result = invoke(*activity, '--json', overrides={'AOC_PRISM_TIMEOUT': '0.15'})
assert not result.stdout and time.monotonic() - start < 1.15
assert 'Timeout' in log.read_text().splitlines()[-1]
start = time.monotonic()
invoke(*activity, '--strict', overrides={'AOC_PRISM_TIMEOUT': '0.15'}, code=1)
assert time.monotonic() - start < 1.15
mode(disconnect=True)
invoke(*activity, '--strict', code=1)
mode(extra_length=20)
invoke(*activity, '--json', '--strict', code=1)
assert ' IncompleteRead 200 ' in log.read_text().splitlines()[-1]
mode()

for command in (activity, notify, ['resolve', '--dedupe-key', 'item:7'], ['status']):
    result = invoke(*command, overrides={'AOC_PRISM': 'off', 'AOC_PRISM_TOKEN': None,
                                         'AOC_PRISM_TOKEN_FILE': str(tmp)}, sent=0)
    assert not result.stdout and ' disabled ' in log.read_text().splitlines()[-1]
invoke(*activity, overrides={'AOC_PRISM_TOKEN': None}, sent=0)
invoke(*activity, '--strict', overrides={'AOC_PRISM_TOKEN': None}, code=1, sent=0)
invoke(*activity, overrides={'AOC_PRISM_TOKEN': None, 'AOC_PRISM_TOKEN_FILE': str(tmp)}, sent=0)
malformed = tmp / 'malformed-token.json'
malformed.write_text('{')
invoke(*activity, '--strict', overrides={'AOC_PRISM_TOKEN': None,
                                        'AOC_PRISM_TOKEN_FILE': str(malformed)}, sent=0, code=1)

# Test file uses only the fake token; never the operator's token file.
token_file = tmp / 'fake-token.json'
token_file.write_text(json.dumps({'token': token, 'id': 'tok_test_id'}))
result = invoke('status', overrides={'AOC_PRISM_TOKEN': None,
                                    'AOC_PRISM_TOKEN_FILE': str(token_file)}, sent=0)
assert json.loads(result.stdout) == {'baseUrl': base, 'tokenFile': str(token_file),
                                    'tokenFound': True, 'tokenId': 'tok_test_id'}
invoke(*activity, overrides={'AOC_PRISM_TOKEN': None, 'AOC_PRISM_TOKEN_FILE': str(token_file)})
invoke(*activity, overrides={'AOC_PRISM_TOKEN': '', 'AOC_PRISM_TOKEN_FILE': str(token_file)}, sent=0)
result = invoke('status', sent=0)
assert json.loads(result.stdout)['tokenId'] is None
assert json.loads(result.stdout)['tokenFound'] is True
result = invoke('status', overrides={'AOC_PRISM_TOKEN': None, 'AOC_PRISM_TOKEN_FILE': None}, sent=0)
assert json.loads(result.stdout)['tokenFile'] == str(tmp / 'home/.config/prism/aoc-token.json')
assert json.loads(result.stdout)['tokenFound'] is False

invoke('activity', '--agent', ' é' + 'é' * 200 + ' ', '--status', 'progress',
       '--summary', ' ' + 'é' * 4001 + ' ')
assert requests()[-1]['body']['agent'] == 'é' * 200
assert requests()[-1]['body']['summary'] == 'é' * 4000
invoke('notify', '--title', ' ' + 'x' * 4001 + ' ', '--body', ' ' + 'é' * 4001 + ' ')
assert requests()[-1]['body']['title'] == 'x' * 4000
assert requests()[-1]['body']['body'] == 'é' * 4000
invoke('activity', '--agent', 'worker', '--status', 'progress', '--summary', 'one\ntwo\r\n' + token)
assert '[redacted]' in log.read_text().splitlines()[-1]
mode(body={'echo': token})
result = invoke(*activity, '--json')
assert json.loads(result.stdout) == {'echo': '[redacted]'}
mode()

for args in (['activity', '--agent', 'worker', '--status', 'started', '--summary', ' \n\t'],
             ['activity', '--agent', ' ', '--status', 'started', '--summary', 'ready'],
             ['notify', '--title', ' ', '--body', 'ready'],
             ['notify', '--title', 'ready', '--body', ' '],
             ['resolve', '--dedupe-key', ' '],
             [*activity, '--issue-url', 'file:///tmp/item'],
             [*notify, '--url', 'ftp://example.test'],
             [*notify, '--issue-url', 'not-a-url']):
    result = invoke(*args, sent=0, logged=False, code=2)
    assert 'error:' in result.stderr and not result.stdout

# Connection refusal, not a real external host, exercises the network-error path.
with socket.socket() as unused:
    unused.bind(('127.0.0.1', 0))
    refused = 'http://127.0.0.1:' + str(unused.getsockname()[1])
invoke(*activity, overrides={'AOC_PRISM_URL': refused}, sent=0)
invoke(*activity, '--strict', overrides={'AOC_PRISM_URL': refused}, sent=0, code=1)
assert ' URLError ' in log.read_text().splitlines()[-1]
# A missing log directory must not stop delivery or expose an exception.
result = subprocess.run([str(cli), *activity], env={**env, 'XDG_STATE_HOME': str(token_file)},
                        text=True, capture_output=True, timeout=5, check=False)
assert result.returncode == 0 and not result.stdout
assert result.stderr == 'aoc-prism: log unavailable\n'
assert token not in result.stderr
print(f'Prism contract cases passed ({checks} CLI calls)')
PY

printf 'AOC Prism smoke passed\n'

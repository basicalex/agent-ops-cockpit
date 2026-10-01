#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

PYTHONDONTWRITEBYTECODE=1 python3 -B - "$root" "$tmp_dir" <<'PY'
import contextlib
import datetime as dt
import fcntl
import io
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import sys
import time

root, tmp = map(Path, sys.argv[1:])
tmp = tmp.resolve()  # macOS git reports /private/var, not the /var symlink.
fake_bin = tmp / 'bin'
fake_bin.mkdir()
state_path = tmp / 'herdr.json'
log_path = tmp / 'master-launch.log'
herdr = fake_bin / 'herdr'
herdr.write_text(r'''#!/usr/bin/env python3
import datetime as dt
import fcntl
import json
import os
from pathlib import Path
import re
import shlex
import sys

path = Path(os.environ['HERDR_FAKE_STATE'])
args = sys.argv[1:]
session = None
if args[:1] == ['--session']:
    session = args[1]
    args = args[2:]
if os.environ.get('HERDR_EXPECT_SESSION'):
    assert session == os.environ['HERDR_EXPECT_SESSION'], (session, args)

def opt(name):
    return args[args.index(name) + 1]

with path.with_suffix('.lock').open('a+') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    state = json.loads(path.read_text())
    state['log'].append({'session': session, 'args': args})
    result = {}
    text = None
    code = 0
    if not args:
        text = 'fake Herdr attach'
    elif args == ['status', 'server']:
        # A successful CLI exit alone does not prove a running server.
        text = 'status: running' if state['up'] else 'status: stopped'
        if not state['up']:
            code = int(os.environ.get('HERDR_FAKE_STOP_EXIT', '0'))
    elif args[:2] == ['workspace', 'list']:
        result = {'workspaces': state['workspaces']}
    elif args[:2] == ['tab', 'list']:
        result = {'tabs': [t for t in state['tabs'] if t['workspace_id'] == opt('--workspace')]}
    elif args[:2] == ['pane', 'list']:
        wid = opt('--workspace') if '--workspace' in args else None
        result = {'panes': [p for p in state['panes'] if wid is None or p['workspace_id'] == wid]}
    elif args[:2] == ['tab', 'create']:
        assert '--no-focus' in args and opt('--label') == 'Master', args
        wid = opt('--workspace')
        n = len(state['tabs']) + 1
        tab = {'workspace_id': wid, 'tab_id': f'{wid}:t{n}', 'label': opt('--label')}
        pane = {'workspace_id': wid, 'tab_id': tab['tab_id'], 'pane_id': f'{wid}:p{n}',
                'terminal_id': f'term{n}', 'cwd': opt('--cwd'), 'agent': None, 'output': '$ '}
        state['tabs'].append(tab)
        state['panes'].append(pane)
        result = {'tab': tab}
    elif args[:2] == ['pane', 'read']:
        pane = next(p for p in state['panes'] if p['pane_id'] == args[2])
        text = pane['output']
    elif args[:2] == ['pane', 'run']:
        pane = next(p for p in state['panes'] if p['pane_id'] == args[2])
        tab = next(t for t in state['tabs'] if t['tab_id'] == pane['tab_id'])
        assert tab['label'].lower() == 'master', (pane, args)
        command = shlex.split(args[3])
        assert command[:3] == ['aoc-dispatch', 'seat', '--root'] and len(command) == 4, command
        state['runs'].append({'pane_id': pane['pane_id'], 'root': command[3], 'command': args[3]})
        seats = Path(os.environ['XDG_STATE_HOME']) / 'aoc/master/seats'
        seats.mkdir(parents=True, exist_ok=True)
        key = re.sub(r'[^A-Za-z0-9_.-]', '_', pane['pane_id'])
        heartbeat = {'schema': 'aoc.master.seat/v1', 'pid': int(os.environ['AOC_TEST_SEAT_PID']),
                     'pane_id': pane['pane_id'], 'root': command[3], 'inbox': 'test/repo',
                     'role': 'owner', 'seat': 'IDLE', 'updated_at': dt.datetime.now(dt.timezone.utc).isoformat()}
        (seats / f'{key}.json').write_text(json.dumps(heartbeat))
        pane['agent'] = 'seat'
        pane['output'] = 'dispatcher view'
    elif args[:2] in (['workspace', 'focus'], ['workspace', 'create']):
        pass  # Launch wrapper only; manager is forbidden to call these.
    else:
        raise AssertionError(args)
    path.write_text(json.dumps(state))
    print(text if text is not None else json.dumps({'result': result}))
    sys.exit(code)
''')
herdr.chmod(0o755)
for name in ('gh', 'claude'):
    forbidden = fake_bin / name
    forbidden.write_text('#!/bin/sh\necho "forbidden real integration" >&2\nexit 99\n')
    forbidden.chmod(0o755)

os.environ.update({
    'PATH': str(fake_bin) + os.pathsep + os.environ['PATH'],
    'AOC_MASTER_HERDR_BIN': str(herdr), 'HERDR_FAKE_STATE': str(state_path),
    'HERDR_EXPECT_SESSION': 'test', 'XDG_STATE_HOME': str(tmp / 'state'),
    'AOC_TEST_SEAT_PID': str(os.getpid()), 'AOC_MASTER_TICK': '0.02',
    'AOC_MASTER_SERVER_GONE_AFTER': '0.15', 'AOC_MASTER_SERVER_WAIT': '0.3',
})
manager_type = runpy.run_path(str(root / 'bin/aoc-master'))['Manager']
manager = manager_type('test')
cli = [sys.executable, '-B', str(root / 'bin/aoc-master')]


def call(*args, env=None):
    result = subprocess.run([*cli, *args], env=env, capture_output=True, text=True, timeout=5)
    assert result.returncode == 0, (args, result.stdout, result.stderr)
    return result.stdout


def load():
    with state_path.with_suffix('.lock').open('a+') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        return json.loads(state_path.read_text())


def change(callback):
    with state_path.with_suffix('.lock').open('a+') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        state = json.loads(state_path.read_text())
        callback(state)
        state_path.write_text(json.dumps(state))


def await_condition(predicate, timeout=5):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.02)
    raise AssertionError('condition timed out')


def beat_path(pane):
    return manager.directory / 'seats' / (pane.replace(':', '_') + '.json')


project = tmp / 'git-project'
project.mkdir()
subprocess.run(['git', 'init', '-q', str(project)], check=True, capture_output=True)
subdir = project / 'nested'
subdir.mkdir()
non_git = tmp / "non git's folder"
non_git.mkdir()
state = {'up': True, 'workspaces': [], 'tabs': [], 'panes': [], 'runs': [], 'log': []}


def workspace(wid, label, cwd, master=None, agent=None, output='$ '):
    state['workspaces'].append({'workspace_id': wid, 'label': label})
    state['tabs'].append({'workspace_id': wid, 'tab_id': wid + ':first', 'label': 'Editor'})
    state['panes'].append({'workspace_id': wid, 'tab_id': wid + ':first', 'pane_id': wid + ':base', 'cwd': str(cwd)})
    if master:
        state['tabs'].append({'workspace_id': wid, 'tab_id': wid + ':master', 'label': master})
        state['panes'].append({'workspace_id': wid, 'tab_id': wid + ':master', 'pane_id': wid + ':seat',
                               'cwd': str(cwd), 'agent': agent, 'output': output})


workspace('w1', 'git', subdir)
workspace('w2', 'non-git', non_git)
workspace('w3', 'adopt', non_git, master='master', output='old output\n\x1b[32muser@host ~/project %\x1b[0m ')
workspace('w4', 'busy', non_git, master='MASTER', agent='claude')
workspace('w5', 'duplicates', non_git, master='Master')
state['tabs'].append({'workspace_id': 'w5', 'tab_id': 'w5:duplicate', 'label': 'master'})
state['panes'].append({'workspace_id': 'w5', 'tab_id': 'w5:duplicate', 'pane_id': 'w5:other', 'cwd': str(non_git), 'agent': None, 'output': '$ '})
workspace('w6', 'AOC Services · excluded', non_git)
workspace('w7', 'non-prompt', non_git, master='Master', output='still working\n> ')
state_path.write_text(json.dumps(state))

output = io.StringIO()
with contextlib.redirect_stdout(output):
    manager.reconcile()
    manager.reconcile()
observed = load()
assert len(observed['runs']) == 4, observed['runs']
assert {r['pane_id'].split(':')[0] for r in observed['runs']} == {'w1', 'w2', 'w3', 'w5'}
assert next(r['root'] for r in observed['runs'] if r['pane_id'].startswith('w1:')) == str(project)
assert next(r['root'] for r in observed['runs'] if r['pane_id'].startswith('w2:')) == str(non_git)
for wid in ('w1', 'w2'):
    assert len([t for t in observed['tabs'] if t['workspace_id'] == wid and t['label'].lower() == 'master']) == 1
assert not any(t['workspace_id'] == 'w6' and t['label'].lower() == 'master' for t in observed['tabs'])
assert len([t for t in observed['tabs'] if t['workspace_id'] == 'w5' and t['label'].lower() == 'master']) == 2
assert output.getvalue().count('Master tab in w4 is busy; seat not started') == 1
assert output.getvalue().count('Duplicate Master tabs in w5:') == 1
assert not any(entry['args'][:2] in (['tab', 'close'], ['pane', 'close'], ['workspace', 'create'], ['workspace', 'focus']) for entry in observed['log'])

# Heartbeat age and PID liveness both matter; stale manager-created panes restart even with an agent set.
managed_pane = next(r['pane_id'] for r in observed['runs'] if r['pane_id'].startswith('w1:'))
heartbeat_path = beat_path(managed_pane)
heartbeat = json.loads(heartbeat_path.read_text())
heartbeat['updated_at'] = (dt.datetime.now(dt.timezone.utc) - dt.timedelta(seconds=61)).isoformat()
heartbeat_path.write_text(json.dumps(heartbeat))
manager = manager_type('test')  # Ownership survives a manager restart.
with contextlib.redirect_stdout(io.StringIO()):
    manager.reconcile()
assert len(load()['runs']) == 5
assert load()['runs'][-1]['pane_id'] == managed_pane
heartbeat = json.loads(heartbeat_path.read_text())
heartbeat['pid'] = -1
heartbeat_path.write_text(json.dumps(heartbeat))
assert manager.heartbeat(managed_pane)[1] is False
with contextlib.redirect_stdout(io.StringIO()):
    manager.reconcile()
assert len(load()['runs']) == 6
status = json.loads(call('--session', 'test', 'status', '--json'))
assert status['running'] is False
rows = {r['workspace_id']: r for r in status['workspaces']}
assert rows['w1']['root'] == str(project) and rows['w1']['seat_alive'] is True
assert rows['w1']['role'] == 'owner' and rows['w1']['seat'] == 'IDLE'
assert rows['w4']['seat_alive'] is False and 'w6' not in rows
heartbeat = json.loads(heartbeat_path.read_text())
heartbeat.update(role='no-inbox', inbox=None, seat=None)
heartbeat_path.write_text(json.dumps(heartbeat))
with contextlib.redirect_stdout(io.StringIO()):
    manager.reconcile()
assert json.loads(manager.state_path.read_text())['workspaces']['w1']['note'] == 'no inbox configured'

# No heartbeat yet: do not retype into a newly started seat while it is initializing.
heartbeat_path.unlink()
with contextlib.redirect_stdout(io.StringIO()):
    manager.reconcile()
assert len(load()['runs']) == 6

# Actual detached CLI, singleton lock, stop, fresh-start wait, and server-gone loop.
change(lambda s: s.update(workspaces=[], tabs=[], panes=[], up=True))
try:
    start = time.monotonic()
    call('start', '--session', 'test')
    assert time.monotonic() - start < 1.5
    start = time.monotonic()
    assert call('start', '--session', 'test').strip() == 'aoc-master already running'
    assert time.monotonic() - start < 1
    assert json.loads(call('status', '--session', 'test', '--json'))['running'] is True
    # Foreground run also refuses a held lock without touching Herdr.
    before = len(load()['log'])
    call('run', '--session', 'test')
    after = load()['log'][before:]
    assert not any(e['args'][:2] == ['tab', 'create'] for e in after)
    existing_seats = {p.name: p.read_text() for p in (manager.directory / 'seats').glob('*.json')}
    call('stop', '--session', 'test')
    await_condition(lambda: not manager.running())
    assert existing_seats == {p.name: p.read_text() for p in (manager.directory / 'seats').glob('*.json')}

    # Initial absence waits for startup, not the shorter server-gone interval.
    change(lambda s: s.update(up=False))
    longer_wait = dict(os.environ, AOC_MASTER_SERVER_WAIT='1')
    call('start', '--session', 'test', env=longer_wait)
    time.sleep(0.25)
    assert manager.running()
    change(lambda s: s.update(up=True))
    await_condition(lambda: any(e['args'] == ['workspace', 'list'] for e in load()['log'][-6:]))
    change(lambda s: s.update(up=False))
    await_condition(lambda: not manager.running())

    # Foreground run exits after continuous loss, using short test intervals.
    change(lambda s: s.update(up=True, log=[]))
    child = subprocess.Popen([*cli, 'run', '--session', 'test'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        await_condition(lambda: any(e['args'] == ['workspace', 'list'] for e in load()['log']))
        change(lambda s: s.update(up=False))
        start = time.monotonic()
        stdout, stderr = child.communicate(timeout=5)
        assert child.returncode == 0, (stdout, stderr)
        assert 0.1 <= time.monotonic() - start < 2
    finally:
        if child.poll() is None:
            child.terminate()
            child.wait(timeout=5)
    # Never-seen server exits at the startup deadline too.
    start = time.monotonic()
    call('run', '--session', 'test')
    assert 0.25 <= time.monotonic() - start < 2
finally:
    call('stop', '--session', 'test')
    await_condition(lambda: not manager.running())

# Copy the actual wrapper to exercise PATH fallback without selecting the real sibling manager.
launcher_dir = tmp / 'launcher'
launcher_dir.mkdir()
launcher = launcher_dir / 'aoc-herdr-launch'
shutil.copy2(root / 'bin/aoc-herdr-launch', launcher)
fake_master = fake_bin / 'aoc-master'
fake_master.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$AOC_TEST_LAUNCH_LOG"\n')
fake_master.chmod(0o755)
launch_env = dict(os.environ, AOC_TEST_LAUNCH_LOG=str(log_path), AOC_HERDR_SERVICES='off', HERDR_FAKE_STOP_EXIT='1')
launch_env.pop('HERDR_EXPECT_SESSION')
launch_env.pop('AOC_HERDR_MASTER', None)
for up in (True, False):
    change(lambda s: s.update(up=up))
    subprocess.run([str(launcher), '--cwd', str(non_git), '--session', 'launch-test'], env=launch_env,
                   check=True, capture_output=True, text=True, timeout=5)
assert log_path.read_text().splitlines() == ['start --session launch-test'] * 2
launch_env['AOC_HERDR_MASTER'] = 'off'
subprocess.run([str(launcher), '--cwd', str(non_git), '--session', 'launch-test'], env=launch_env,
               check=True, capture_output=True, text=True, timeout=5)
assert log_path.read_text().splitlines() == ['start --session launch-test'] * 2

# Sibling takes priority over PATH; startup failure remains non-fatal.
sibling = launcher_dir / 'aoc-master'
sibling.write_text('#!/bin/sh\nprintf "sibling %s\\n" "$*" >> "$AOC_TEST_LAUNCH_LOG"\nexit 1\n')
sibling.chmod(0o755)
launch_env.pop('AOC_HERDR_MASTER')
subprocess.run([str(launcher), '--cwd', str(non_git), '--session', 'launch-test'], env=launch_env,
               check=True, capture_output=True, text=True, timeout=5)
assert log_path.read_text().splitlines()[-1] == 'sibling start --session launch-test'
PY

printf 'AOC Master seat manager smoke passed\n'

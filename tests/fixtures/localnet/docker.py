#!/usr/bin/env python3
"""Docker protocol fixture for CLI lifecycle tests; never starts real containers."""
import json
import os
import sys
import time
from pathlib import Path

args = sys.argv[1:]
root = Path(os.environ['LOCALNET_TEST_DIR'])
marker = root / 'running'
network_marker = marker

if (root / 'docker-unavailable').exists():
    raise SystemExit('Docker daemon is unavailable')

if 'compose' in args:
    compose_path = Path(args[args.index('-f') + 1])
    network_marker = compose_path.parent / 'fixture-running'
    command = args[args.index('-f') + 2:]
    verb = command[0]

    if verb in ['up', 'stop', 'down']:
        with (root / 'network-events').open('a') as output:
            output.write(json.dumps({'network': json.loads((compose_path.parent / 'network.json').read_text())['name'], 'command': verb}) + '\n')

    if verb == 'ps':
        names = ['localton', 'postgres', 'redis', 'v3-basechain-bootstrap', 'v3-migrations',
                 'v3-worker', 'v3-account-scanner', 'v3-api', 'v3-classifier']
        states = []
        for name in names:
            state = 'running' if network_marker.exists() else 'exited'
            health = 'healthy' if network_marker.exists() else ''
            exit_code = 0
            if name in ['v3-basechain-bootstrap', 'v3-migrations']:
                state, health = 'exited', ''
            if network_marker.exists() and os.environ.get('LOCALNET_TEST_BLOCK_START'):
                if name == 'localton':
                    state, health = 'running', 'starting'
                elif name not in ['postgres', 'redis', 'v3-migrations']:
                    # Old containers retain SIGTERM's exit code until Compose
                    # starts them again; startup progress must not report failure.
                    state, health, exit_code = 'exited', '', 143
            states.append({'Service': name, 'State': state, 'Health': health, 'ExitCode': exit_code})
        print(json.dumps(states))
    elif verb == 'up':
        with (root / 'events').open('a') as output:
            output.write('up\n')
        marker.touch()
        network_marker.touch()
        if os.environ.get('LOCALNET_TEST_BLOCK_START'):
            time.sleep(60)
    elif verb in ['stop', 'down']:
        with (root / 'events').open('a') as output:
            output.write(verb + '\n')
        if (root / 'slow-stop').exists():
            time.sleep(1)
        if (root / 'hold-stop').exists():
            (root / 'stop-entered').touch()
            deadline = time.monotonic() + 15
            while (root / 'hold-stop').exists():
                if time.monotonic() > deadline:
                    raise SystemExit('Test did not release Docker stop')
                time.sleep(0.02)
        if (root / 'fail-stop').exists():
            raise SystemExit('Docker could not stop the fixture network')
        network_marker.unlink(missing_ok=True)
        marker.unlink(missing_ok=True)
    elif verb == 'run':
        action = command[command.index('snapshot') + 1]
        snapshot = {'formatVersion': 1, 'id': 'snapshot-1', 'name': 'checkpoint',
                    'createdAt': 1, 'archiveSizeBytes': 100, 'stateSizeBytes': 200,
                    'stateSchemaVersion': 1, 'tonRelease': 'fixture', 'masterchainSeqno': 10}
        with (root / 'events').open('a') as output:
            output.write('snapshot-' + action + '\n')
        print(json.dumps([snapshot] if action == 'list' else None if action == 'delete' else snapshot))
    elif verb == 'exec':
        with (root / 'events').open('a') as output:
            output.write('validation\n')
    else:
        raise SystemExit('Unexpected compose command: ' + verb)
elif 'image' in args and 'inspect' in args:
    if (root / 'force-pull').exists() and not (root / 'image-ready').exists():
        raise SystemExit(1)
    print('fixture-image-id')
elif 'pull' in args:
    print('aaaaaaaaaaaa: Pulling fs layer', flush=True)
    print('bbbbbbbbbbbb: Pulling fs layer', flush=True)
    print('aaaaaaaaaaaa: Pull complete', flush=True)
    print('bbbbbbbbbbbb: Download complete', flush=True)
    deadline = time.monotonic() + 15
    while not (root / 'continue-pull').exists():
        if time.monotonic() > deadline:
            raise SystemExit('Test did not release image pull')
        time.sleep(0.05)
    print('bbbbbbbbbbbb: Pull complete', flush=True)
    (root / 'image-ready').touch()
elif 'volume' in args or 'rm' in args:
    pass
elif 'context' in args and 'show' in args:
    print('localnet-test')
else:
    raise SystemExit('Unexpected Docker command')

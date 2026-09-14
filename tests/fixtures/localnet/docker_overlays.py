#!/usr/bin/env python3
"""Overlay console fixture layered over the regular localnet Docker fixture."""
import base64
import json
import os
import runpy
import sys
from pathlib import Path

args = sys.argv[1:]
root = Path(os.environ['LOCALNET_TEST_DIR'])

if 'compose' in args and '-f' in args:
    compose_path = Path(args[args.index('-f') + 1])
    command = args[args.index('-f') + 2:]
    service = command[2] if command[0] == 'exec' else None
    state_path = compose_path.parent / ('fixture-overlays-' + str(service) + '.json')
    if command[0] == 'exec' and 'custom-overlays.json' in command[-1]:
        print(json.dumps({
            'engine': {'fullnode': base64.b64encode(bytes([1 if service == 'localton' else 2]) * 32).decode()},
            'overlays': json.loads(state_path.read_text()),
        }))
        raise SystemExit(0)
    if command[0] == 'exec' and '-rc' in command:
        query = command[command.index('-rc') + 1]
        if query.startswith('del-custom-overlay '):
            state = json.loads(state_path.read_text())
            name = query.removeprefix('del-custom-overlay ')
            state['overlays'] = [item for item in state['overlays'] if item['name'] != name]
            state_path.write_text(json.dumps(state))
            with (root / 'overlay-events').open('a') as output:
                output.write(query + '\n')
            print('success')
            raise SystemExit(0)
    if command[0] == 'exec' and any('add-custom-overlay $file' in arg for arg in command):
        state = json.loads(state_path.read_text())
        overlay = json.load(sys.stdin)
        state['overlays'].append(overlay)
        state_path.write_text(json.dumps(state))
        with (root / 'overlay-events').open('a') as output:
            output.write('add-custom-overlay ' + overlay['name'] + '\n')
        print('success')
        raise SystemExit(0)

runpy.run_path(str(Path(__file__).with_name('docker-base.py')), run_name='__main__')

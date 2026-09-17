#!/usr/bin/env python3
"""Docker protocol fixture for CLI lifecycle tests; never starts real containers."""
import base64
import json
import os
import re
import struct
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, unquote, urlsplit

root = Path(os.environ['LOCALNET_TEST_DIR'])
containers, networks, volumes, executions = {}, {}, {}, {}
event_lock = threading.Lock()
project_label = 'org.ton.acton.localnet.project'
service_label = 'org.ton.acton.localnet.service'


def event(file, value):
    with event_lock, (root / file).open('a') as output:
        output.write((value if isinstance(value, str) else json.dumps(value)) + '\n')


def location(project):
    for descriptor in (root / '.acton-localnet').rglob('runtime.json'):
        if json.loads(descriptor.read_text())['projectName'] == project:
            return descriptor.parent
    raise RuntimeError('Unknown fixture deployment: ' + project)


def network_event(project, command):
    directory = location(project)
    name = json.loads((directory / 'network.json').read_text())['name']
    event('events', command)
    event('network-events', {'network': name, 'command': command})


def wait_marker(name, entered):
    if (root / name).exists():
        (root / entered).touch()
        deadline = time.monotonic() + 15
        while (root / name).exists():
            if time.monotonic() > deadline:
                raise RuntimeError('Test did not release ' + name)
            time.sleep(0.02)


def matches(labels, filters):
    return all(labels.get(key) == value for key, value in
               (entry.split('=', 1) for entry in filters.get('label', [])))


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *args):
        pass

    def reply(self, value=None, status=200):
        body = json.dumps(value).encode() if value is not None else b''
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def upgrade(self):
        self.send_response(101)
        self.send_header('Connection', 'Upgrade')
        self.send_header('Upgrade', 'tcp')
        self.end_headers()
        self.wfile.flush()
        self.close_connection = True

    def frame(self, value, channel=1):
        data = value if isinstance(value, bytes) else json.dumps(value).encode() + b'\n'
        self.wfile.write(struct.pack('>B3xI', channel, len(data)) + data)
        self.wfile.flush()

    def handle_request(self):
        parsed = urlsplit(self.path)
        path = unquote(re.sub(r'^/v[0-9.]+', '', parsed.path))
        query = parse_qs(parsed.query)
        body = self.rfile.read(int(self.headers.get('Content-Length', 0)))
        data = json.loads(body) if body else {}
        method = self.command
        event('engine-events', {'method': method, 'path': path, 'query': query, 'body': data})
        if (root / 'docker-error').exists():
            return self.reply({'message': (root / 'docker-error').read_text()}, 500)
        if (root / 'docker-unavailable').exists():
            return self.reply({'message': 'Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?'}, 500)
        if path in ['/version', '/_ping']:
            if (root / 'docker-timeout').exists():
                time.sleep(15)
            return self.reply({'ApiVersion': '1.53', 'Version': '29.0.0', 'Os': 'linux', 'Arch': 'arm64'} if path == '/version' else None)
        if path.startswith('/images/') and path.endswith('/json'):
            if (root / 'force-pull').exists() and not (root / 'image-ready').exists():
                return self.reply({'message': 'No such image'}, 404)
            return self.reply({'Id': 'sha256:fixture', 'Config': {'Labels': {'org.ton.localton.admin-hardforks': '1'}}})
        if path == '/images/create':
            if (root / 'require-registry-auth').exists():
                auth = json.loads(base64.b64decode(self.headers.get('X-Registry-Auth', 'e30=')))
                if auth.get('username') != 'fixture-user' or auth.get('password') != 'fixture-token':
                    return self.reply({'message': 'authentication required'}, 401)
                (root / 'registry-auth-accepted').touch()
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Connection', 'close')
            self.end_headers()
            self.close_connection = True
            for id, status in [('aaaaaaaaaaaa', 'Pulling fs layer'), ('bbbbbbbbbbbb', 'Pulling fs layer'),
                               ('aaaaaaaaaaaa', 'Pull complete'), ('bbbbbbbbbbbb', 'Download complete')]:
                self.wfile.write(json.dumps({'id': id, 'status': status}).encode() + b'\n')
            self.wfile.flush()
            deadline = time.monotonic() + 15
            while not (root / 'continue-pull').exists():
                if time.monotonic() > deadline:
                    raise RuntimeError('Test did not release image pull')
                time.sleep(0.02)
            self.wfile.write(b'{"id":"bbbbbbbbbbbb","status":"Pull complete"}\n')
            (root / 'image-ready').touch()
            return
        if path == '/networks/create':
            networks[data['Name']] = {'Id': data['Name'], **data}
            return self.reply({'Id': data['Name'], 'Warning': ''}, 201)
        if path.startswith('/networks/'):
            name = path[len('/networks/'):]
            if name not in networks:
                return self.reply({'message': 'No such network'}, 404)
            if method == 'DELETE':
                network_event(networks.pop(name)['Labels'][project_label], 'down')
                return self.reply(status=204)
            return self.reply(networks[name])
        if path == '/volumes/create':
            volume = {'Name': data['Name'], 'Driver': 'local', 'Mountpoint': '/fixture', 'Labels': data.get('Labels', {}), 'Options': {}, 'Scope': 'local'}
            volumes[data['Name']] = volume
            return self.reply(volume, 201)
        if path == '/volumes':
            filters = json.loads(query.get('filters', ['{}'])[0])
            return self.reply({'Volumes': [v for v in volumes.values() if matches(v['Labels'], filters)], 'Warnings': []})
        if path.startswith('/volumes/'):
            name = path[len('/volumes/'):]
            if name not in volumes:
                return self.reply({'message': 'No such volume'}, 404)
            if method == 'DELETE':
                del volumes[name]
                return self.reply(status=204)
            return self.reply(volumes[name])
        if path == '/containers/json':
            filters = json.loads(query.get('filters', ['{}'])[0])
            return self.reply([{'Id': c['Id'], 'Names': [c['Name']], 'Labels': c['Config']['Labels'],
                                'Image': c['Config']['Image'], 'State': c['State']['Status']}
                               for c in list(containers.values()) if matches(c['Config']['Labels'], filters)])
        if path == '/containers/create':
            name = query.get('name', ['helper-' + str(time.time_ns())])[0]
            if any(c['Name'] == '/' + name for c in containers.values()):
                return self.reply({'message': 'Name conflict'}, 409)
            id = 'container-' + str(time.time_ns())
            containers[id] = {'Id': id, 'Name': '/' + name, 'Config': data, 'HostConfig': data.get('HostConfig'),
                              'State': {'Status': 'created', 'Running': False, 'ExitCode': 0}, 'started': threading.Event()}
            return self.reply({'Id': id, 'Warnings': []}, 201)
        if path.startswith('/containers/'):
            parts = path[len('/containers/'):].split('/')
            name, action = parts[0], parts[1] if len(parts) > 1 else ''
            container = next((c for c in list(containers.values()) if c['Id'] == name or c['Name'] == '/' + name), None)
            if not container:
                return self.reply({'message': 'No such container'}, 404)
            labels = container['Config']['Labels']
            project, service = labels[project_label], labels.get(service_label)
            if action == 'json':
                inspection = {k: v for k, v in container.items() if k != 'started'}
                if service and (location(project) / 'fixture-force-running').exists():
                    one_shot = service in ['v3-migrations', 'v3-basechain-bootstrap']
                    inspection['State'] = {'Status': 'exited' if one_shot else 'running',
                                           'Running': not one_shot, 'ExitCode': 0}
                return self.reply(inspection)
            if action == 'start':
                if service and service.startswith('node-') and (root / 'fail-node-operation').exists():
                    return self.reply({'message': 'Fixture node operation failed'}, 500)
                container['State'] = {'Status': 'running', 'Running': True, 'ExitCode': 0}
                if service:
                    directory = location(project)
                    if service == 'localton':
                        network_event(project, 'up')
                        (directory / 'fixture-running').touch()
                        (root / 'running').touch()
                    elif service.startswith('node-'):
                        event('node-events', {'node': service, 'command': 'start'})
                        (directory / ('fixture-running-' + service)).touch()
                    if service in ['v3-migrations', 'v3-basechain-bootstrap']:
                        container['State'] = {'Status': 'exited', 'Running': False, 'ExitCode': 0}
                    elif container['Config'].get('Healthcheck', {}).get('Test') != ['NONE']:
                        container['State']['Health'] = {'Status': 'starting' if service == 'localton' and (root / 'block-start').exists() else 'healthy'}
                container['started'].set()
                return self.reply(status=204)
            if action == 'stop':
                if service and service.startswith('node-') and (root / 'fail-node-operation').exists():
                    return self.reply({'message': 'Fixture node operation failed'}, 500)
                if (root / 'slow-stop').exists():
                    time.sleep(1)
                wait_marker('hold-stop', 'stop-entered')
                if (root / 'fail-stop').exists():
                    return self.reply({'message': 'Docker could not stop the fixture network'}, 500)
                if service == 'localton':
                    network_event(project, 'stop')
                    (location(project) / 'fixture-running').unlink(missing_ok=True)
                    (root / 'running').unlink(missing_ok=True)
                elif service and service.startswith('node-'):
                    event('node-events', {'node': service, 'command': 'stop'})
                    (location(project) / ('fixture-running-' + service)).unlink(missing_ok=True)
                container['State'] = {'Status': 'exited', 'Running': False, 'ExitCode': 143}
                return self.reply(status=204)
            if action == 'attach':
                self.upgrade()
                container['started'].wait(15)
                command = container['Config']['Cmd']
                if command[0] != 'snapshot':
                    raise RuntimeError('Unexpected offline command')
                action = command[1]
                if action == 'create':
                    wait_marker('hold-snapshot', 'snapshot-entered')
                snapshot = {'formatVersion': 2, 'id': 'snapshot-1', 'name': 'checkpoint', 'createdAt': 1,
                            'archiveSizeBytes': 100, 'stateSizeBytes': 200, 'stateSchemaVersion': 1,
                            'tonRelease': 'fixture', 'masterchainSeqno': 10}
                event('events', 'snapshot-' + action)
                self.frame([snapshot] if action == 'list' else None if action == 'delete' else snapshot)
                container['State'] = {'Status': 'exited', 'Running': False, 'ExitCode': 0}
                return
            if action == 'wait':
                return self.reply({'StatusCode': container['State']['ExitCode']})
            if action == 'exec':
                id = 'exec-' + str(time.time_ns())
                executions[id] = {'Id': id, 'Running': False, 'ExitCode': 0, 'command': data}
                return self.reply({'Id': id}, 201)
            if action == 'logs':
                return self.reply()
            if method == 'DELETE':
                del containers[container['Id']]
                return self.reply(status=204)
        if path.startswith('/exec/'):
            id, action = path[len('/exec/'):].split('/')
            if action == 'json':
                return self.reply({k: v for k, v in executions[id].items() if k != 'command'})
            if action == 'start':
                self.upgrade()
                event('events', 'validation')
                self.frame(b'')
                return
        self.reply({'message': 'Unexpected Engine request: ' + method + ' ' + path}, 501)

    def do_GET(self):
        self.handle_request()

    def do_POST(self):
        self.handle_request()

    def do_DELETE(self):
        self.handle_request()


server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
(root / 'docker-host').write_text('tcp://127.0.0.1:' + str(server.server_port))
server.serve_forever()

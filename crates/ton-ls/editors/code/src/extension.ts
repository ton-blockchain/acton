import * as vscode from 'vscode';
import * as net from 'net';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    Executable,
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: vscode.ExtensionContext) {
    const config = vscode.workspace.getConfiguration('tolk');
    const serverPath = config.get<string>('serverPath') || 'acton';
    const serverPort = config.get<number>('serverPort') || 0;

    let serverOptions: ServerOptions;

    if (serverPort > 0) {
        // TCP debugging mode
        serverOptions = () => {
            return new Promise((resolve, reject) => {
                const client = new net.Socket();
                client.connect(serverPort, '127.0.0.1', () => {
                    resolve({
                        reader: client,
                        writer: client
                    });
                });
                client.on('error', (err) => {
                    reject(err);
                });
            });
        };
    } else {
        // Standard executable mode
        const run: Executable = {
            command: serverPath,
            args: ["ls", "--stdio"],
            options: {
                env: {
                    ...process.env,
                    RUST_LOG: 'debug',
                },
            },
        };

        serverOptions = {
            run,
            debug: run,
        };
    }

    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            { scheme: 'file', language: 'tolk' },
            { scheme: 'file', language: 'tasm' },
            { scheme: 'file', language: 'fift' },
            { scheme: 'file', language: 'tlb' },
            { scheme: 'file', language: 'toml', pattern: '**/Acton.toml' },
            { scheme: 'file', pattern: '**/Acton.toml' },
        ],
        synchronize: {
            fileEvents: [
                vscode.workspace.createFileSystemWatcher('**/*.{tolk,tasm,fif,fift,tlb}'),
                vscode.workspace.createFileSystemWatcher('**/Acton.toml'),
            ],
        },
    };

    client = new LanguageClient(
        'ton-ls',
        'TON Language Server',
        serverOptions,
        clientOptions
    );

    client.start();
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}

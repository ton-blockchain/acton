import * as net from "net"
import * as vscode from "vscode"
import {
  Executable,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node"

let client: LanguageClient | undefined

const typeAtPositionRequest = "tolk.getTypeAtPosition"

interface TypeAtPositionParams {
  textDocument: {uri: string}
  position: {line: number; character: number}
}

interface TypeAtPositionResponse {
  type: string | null
  range: {
    start: {line: number; character: number}
    end: {line: number; character: number}
  } | null
}

export function activate(context: vscode.ExtensionContext) {
  const config = vscode.workspace.getConfiguration("acton.languageServer")
  const serverPath = config.get<string>("path") || "acton"
  const serverArgs = config.get<string[]>("args") || ["ls", "--stdio"]
  const serverPort = config.get<number>("port") || 0
  const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath

  const serverOptions =
    serverPort > 0
      ? connectToServer(serverPort)
      : launchServer(serverPath, serverArgs, cwd)

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      {scheme: "file", language: "tolk"},
      {scheme: "file", language: "tasm"},
      {scheme: "file", language: "fift"},
      {scheme: "file", language: "tlb"},
      {scheme: "file", language: "toml", pattern: "**/Acton.toml"},
      {scheme: "file", pattern: "**/Acton.toml"},
    ],
    synchronize: {
      fileEvents: [
        vscode.workspace.createFileSystemWatcher("**/*.{tolk,tasm,fif,fift,tlb}"),
        vscode.workspace.createFileSystemWatcher("**/Acton.toml"),
      ],
    },
  }

  client = new LanguageClient(
    "acton-language-server",
    "Acton Language Server",
    serverOptions,
    clientOptions,
  )

  client.start()

  context.subscriptions.push(
    vscode.commands.registerCommand(
      typeAtPositionRequest,
      async (params?: TypeAtPositionParams): Promise<TypeAtPositionResponse | null> => {
        const languageClient = client
        if (!languageClient) {
          return null
        }

        const activeEditor = vscode.window.activeTextEditor
        const invokedFromEditor = params === undefined
        if (!params) {
          if (!activeEditor) {
            return null
          }

          params = {
            textDocument: {uri: activeEditor.document.uri.toString()},
            position: {
              line: activeEditor.selection.active.line,
              character: activeEditor.selection.active.character,
            },
          }
        }

        const result = await languageClient.sendRequest<TypeAtPositionResponse>(
          typeAtPositionRequest,
          params,
        )

        if (invokedFromEditor && result.type) {
          if (activeEditor && result.range) {
            const range = new vscode.Range(
              new vscode.Position(result.range.start.line, result.range.start.character),
              new vscode.Position(result.range.end.line, result.range.end.character),
            )
            activeEditor.selection = new vscode.Selection(range.start, range.end)
            activeEditor.revealRange(range)
          }

          await vscode.window.showInformationMessage(`Type: ${result.type}`)
        }

        return result
      },
    ),
  )
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop()
}

function launchServer(
  command: string,
  args: string[],
  cwd: string | undefined,
): ServerOptions {
  const run: Executable = {
    command,
    args,
    options: {
      cwd,
      env: {
        ...process.env,
      },
    },
  }

  return {
    run,
    debug: run,
  }
}

function connectToServer(port: number): ServerOptions {
  return () =>
    new Promise((resolve, reject) => {
      const socket = new net.Socket()
      socket.connect(port, "127.0.0.1", () => {
        resolve({
          reader: socket,
          writer: socket,
        })
      })
      socket.on("error", reject)
    })
}

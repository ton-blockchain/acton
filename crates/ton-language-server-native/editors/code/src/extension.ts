import * as net from "net"
import * as path from "path"
import * as vscode from "vscode"
import {
  Executable,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node"

let client: LanguageClient | undefined

const typeAtPositionRequest = "tolk.getTypeAtPosition"
const openFileCommand = "ton.openFile"

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
    vscode.commands.registerCommand(
      openFileCommand,
      async (filePath: string, line?: string | number): Promise<void> => {
        try {
          const uri = await resolveWorkspaceFile(filePath)
          const document = await vscode.workspace.openTextDocument(uri)
          const editor = await vscode.window.showTextDocument(document)
          const requestedLine = typeof line === "string" ? Number.parseInt(line, 10) : line

          if (requestedLine !== undefined && Number.isFinite(requestedLine)) {
            const lineNumber = Math.min(
              Math.max(requestedLine - 1, 0),
              Math.max(document.lineCount - 1, 0),
            )
            const position = new vscode.Position(lineNumber, 0)
            editor.selection = new vscode.Selection(position, position)
            editor.revealRange(
              new vscode.Range(position, position),
              vscode.TextEditorRevealType.InCenterIfOutsideViewport,
            )
          }
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error)
          await vscode.window.showErrorMessage(`Failed to open ${filePath}: ${message}`)
        }
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

async function resolveWorkspaceFile(filePath: string): Promise<vscode.Uri> {
  if (path.isAbsolute(filePath)) {
    return vscode.Uri.file(filePath)
  }

  const workspaceFolders = vscode.workspace.workspaceFolders ?? []
  for (const folder of workspaceFolders) {
    const candidate = vscode.Uri.joinPath(folder.uri, ...filePath.split(/[\\/]/))
    try {
      await vscode.workspace.fs.stat(candidate)
      return candidate
    } catch {
      // Try the other workspace folders before falling back to a basename search.
    }
  }

  const matches = await vscode.workspace.findFiles(
    `**/${path.basename(filePath)}`,
    "**/node_modules/**",
    20,
  )
  const normalizedPath = filePath.replace(/\\/g, "/")
  const exact = matches.find(uri => uri.path.endsWith(normalizedPath))
  const resolved = exact ?? matches[0]
  if (!resolved) {
    throw new Error("file not found in the workspace")
  }

  return resolved
}

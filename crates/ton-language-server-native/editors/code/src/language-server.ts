import * as path from "path"
import * as vscode from "vscode"
import {
  type Executable,
  LanguageClient,
  type LanguageClientOptions,
  RevealOutputChannelOn,
  type ServerOptions,
} from "vscode-languageclient/node"

import {createClientLog} from "./client-log"
import process from "node:process"

let client: LanguageClient | undefined
let clientCommandsRegistered = false
let restartOperation: Promise<void> = Promise.resolve()

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

export async function startLanguageServer(context: vscode.ExtensionContext): Promise<void> {
  const config = vscode.workspace.getConfiguration("ton")
  const serverPath = config.get<string>("acton.path", "acton")
  const serverArgs = resolveServerArgs(config)
  const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath

  const clientOptions: LanguageClientOptions = {
    outputChannel: createClientLog(),
    revealOutputChannelOn: RevealOutputChannelOn.Never,
    documentSelector: [
      {scheme: "file", language: "tolk"},
      {scheme: "file", language: "func"},
      {scheme: "file", language: "tasm"},
      {scheme: "file", language: "fift"},
      {scheme: "file", language: "tlb"},
      {scheme: "file", language: "toml", pattern: "**/Acton.toml"},
      {scheme: "file", pattern: "**/Acton.toml"},
      {scheme: "untitled", language: "tolk"},
      {scheme: "untitled", language: "func"},
      {scheme: "untitled", language: "tasm"},
      {scheme: "untitled", language: "fift"},
      {scheme: "untitled", language: "tlb"},
    ],
    synchronize: {
      configurationSection: "ton",
      fileEvents: [
        vscode.workspace.createFileSystemWatcher("**/*.{tolk,fc,func,tasm,fif,fift,tlb}"),
        vscode.workspace.createFileSystemWatcher("**/Acton.toml"),
      ],
    },
  }

  client = new LanguageClient(
    "acton-language-server",
    "Acton Language Server",
    launchServer(serverPath, serverArgs, cwd),
    clientOptions,
  )

  await client.start()

  if (clientCommandsRegistered) {
    return
  }
  clientCommandsRegistered = true
  context.subscriptions.push(
    vscode.commands.registerCommand(
      typeAtPositionRequest,
      async (params?: unknown): Promise<TypeAtPositionResponse | null> => {
        const languageClient = client
        if (!languageClient) {
          return null
        }

        const activeEditor = vscode.window.activeTextEditor
        const hasExplicitParams = isTypeAtPositionParams(params)
        const requestParams = hasExplicitParams
          ? params
          : activeEditor
            ? {
                textDocument: {uri: activeEditor.document.uri.toString()},
                position: {
                  line: activeEditor.selection.active.line,
                  character: activeEditor.selection.active.character,
                },
              }
            : undefined
        if (!requestParams) {
          return null
        }

        const result = await languageClient.sendRequest<TypeAtPositionResponse>(
          typeAtPositionRequest,
          requestParams,
        )

        if (!hasExplicitParams && result.type) {
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
    vscode.commands.registerCommand("ton.copyToClipboard", async (value: string) => {
      await vscode.env.clipboard.writeText(value)
      await vscode.window.showInformationMessage(`Copied ${value} to clipboard`)
    }),
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
    new vscode.Disposable(() => {
      void client?.stop()
    }),
  )
}

function isTypeAtPositionParams(value: unknown): value is TypeAtPositionParams {
  if (typeof value !== "object" || value === null) {
    return false
  }

  const params = value as Partial<TypeAtPositionParams>
  return (
    typeof params.textDocument?.uri === "string" &&
    typeof params.position?.line === "number" &&
    typeof params.position.character === "number"
  )
}

export async function stopLanguageServer(): Promise<void> {
  const runningClient = client
  client = undefined
  if (!runningClient) {
    return
  }

  try {
    await runningClient.stop()
  } finally {
    await runningClient.dispose()
  }
}

export function restartLanguageServer(context: vscode.ExtensionContext): Promise<void> {
  const nextRestart = restartOperation
    .catch(() => {})
    .then(async () => {
      await stopLanguageServer()
      await startLanguageServer(context)
    })
  restartOperation = nextRestart
  return nextRestart
}

export async function sendLanguageServerRequest<T>(method: string, params?: unknown): Promise<T> {
  const languageClient = client
  if (!languageClient) {
    throw new Error("Acton language server is not running")
  }
  return languageClient.sendRequest<T>(method, params)
}

function resolveServerArgs(config: vscode.WorkspaceConfiguration): string[] {
  const args = [...config.get<string[]>("languageServer.args", ["ls", "--stdio"])]
  const stdlibPath = config.get<string>("tolk.stdlib.path")?.trim()
  if (
    stdlibPath &&
    !args.some(arg => arg === "--stdlib-path" || arg.startsWith("--stdlib-path="))
  ) {
    args.push("--stdlib-path", stdlibPath)
  }
  if (
    config.get<boolean>("languageServer.profiling.enabled", false) &&
    !args.includes("--profile")
  ) {
    args.push("--profile")
  }
  return args
}

function launchServer(command: string, args: string[], cwd: string | undefined): ServerOptions {
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

import {
  BrowserMessageReader,
  BrowserMessageWriter,
  createConnection,
  TextDocuments,
  TextDocumentSyncKind,
  type InitializeResult,
} from "vscode-languageserver/browser"
import {TextDocument} from "vscode-languageserver-textdocument"

import {TASM_SPEC_URL} from "./languages/tasm"
import init, {TonLanguageServer} from "./wasm/ton_language_server_wasm"

const workerSelf = globalThis as unknown as DedicatedWorkerGlobalScope
const connection = createConnection(
  new BrowserMessageReader(workerSelf),
  new BrowserMessageWriter(workerSelf),
)
const documents = new TextDocuments(TextDocument)

const LOG_LEVEL_REQUEST = "ton/setLogLevel"
const LOGS_REQUEST = "ton/logs"
const CLEAR_LOGS_REQUEST = "ton/clearLogs"
const PROFILE_REQUEST = "ton/profile"
const ADD_SOURCE_FILE_REQUEST = "ton/addSourceFile"
const SET_WORKSPACE_CONFIG_REQUEST = "ton/setWorkspaceConfig"
const STACK_EFFECT_CODE_LENS_COMMAND = "tonls.tasm.stackEffect"

let languageServerPromise: Promise<TonLanguageServer> | undefined

const getLanguageServer = async () => {
  if (!languageServerPromise) {
    languageServerPromise = init().then(async () => {
      const response = await fetch(TASM_SPEC_URL)
      if (!response.ok) {
        throw new Error(`Failed to load TASM specification: ${response.status}`)
      }
      return TonLanguageServer.withTasmSpec(await response.text())
    })
  }
  return languageServerPromise
}

connection.onInitialize(async (): Promise<InitializeResult> => {
  const server = await getLanguageServer()
  return {
    capabilities: {
      definitionProvider: true,
      referencesProvider: true,
      hoverProvider: true,
      completionProvider: {
        resolveProvider: false,
        triggerCharacters: [".", '"', "'", "/", "@"],
      },
      inlayHintProvider: true,
      semanticTokensProvider: {
        legend: {
          tokenTypes: server.semanticTokenTypes() as string[],
          tokenModifiers: server.semanticTokenModifiers() as string[],
        },
        full: true,
        range: false,
      },
      codeLensProvider: {
        resolveProvider: false,
      },
      foldingRangeProvider: true,
      executeCommandProvider: {
        commands: [STACK_EFFECT_CODE_LENS_COMMAND],
      },
      textDocumentSync: TextDocumentSyncKind.Full,
    },
  }
})

documents.onDidOpen(async event => {
  await withLanguageServer("textDocument/didOpen", server =>
    server.openDocument(
      event.document.uri,
      event.document.languageId,
      event.document.version,
      event.document.getText(),
    ),
  )
})

documents.onDidChangeContent(async event => {
  await withLanguageServer("textDocument/didChange", server =>
    server.changeDocument(event.document.uri, event.document.version, event.document.getText()),
  )
})

connection.onDefinition(async params =>
  withLanguageServer("textDocument/definition", server =>
    server.definition(params.textDocument.uri, params.position.line, params.position.character),
  ),
)

connection.onReferences(async params =>
  withLanguageServer("textDocument/references", server =>
    server.references(
      params.textDocument.uri,
      params.position.line,
      params.position.character,
      false,
    ),
  ),
)

connection.onHover(async params =>
  withLanguageServer("textDocument/hover", server =>
    server.hover(params.textDocument.uri, params.position.line, params.position.character),
  ),
)

connection.onCompletion(async params =>
  withLanguageServer("textDocument/completion", server =>
    server.completion(
      params.textDocument.uri,
      params.position.line,
      params.position.character,
      params.context?.triggerKind ?? 1,
      params.context?.triggerCharacter ?? "",
    ),
  ),
)

connection.languages.semanticTokens.on(async params =>
  withLanguageServer("textDocument/semanticTokens/full", server =>
    server.semanticTokens(params.textDocument.uri),
  ),
)

connection.languages.inlayHint.on(async params =>
  withLanguageServer("textDocument/inlayHint", server =>
    server.inlayHints(
      params.textDocument.uri,
      params.range.start.line,
      params.range.start.character,
      params.range.end.line,
      params.range.end.character,
    ),
  ),
)

connection.onCodeLens(async params =>
  withLanguageServer("textDocument/codeLens", server => server.codeLens(params.textDocument.uri)),
)

connection.onFoldingRanges(async params =>
  withLanguageServer("textDocument/foldingRange", server =>
    server.foldingRanges(params.textDocument.uri),
  ),
)

connection.onExecuteCommand(async params => {
  if (params.command === STACK_EFFECT_CODE_LENS_COMMAND) {
    return null
  }
  throw new Error(`Unsupported command: ${params.command}`)
})

connection.onRequest(LOG_LEVEL_REQUEST, async level =>
  withLanguageServer(LOG_LEVEL_REQUEST, server => {
    server.setLogLevel(String(level))
    return server.logs()
  }),
)

connection.onRequest(LOGS_REQUEST, async () =>
  withLanguageServer(LOGS_REQUEST, server => server.logs()),
)

connection.onRequest(CLEAR_LOGS_REQUEST, async () =>
  withLanguageServer(CLEAR_LOGS_REQUEST, server => {
    server.clearLogs()
    return server.logs()
  }),
)

connection.onRequest(PROFILE_REQUEST, async () =>
  withLanguageServer(PROFILE_REQUEST, server => server.profileSummary()),
)

connection.onRequest(ADD_SOURCE_FILE_REQUEST, async params =>
  withLanguageServer(ADD_SOURCE_FILE_REQUEST, server => {
    if (!isRecord(params)) {
      throw new Error("expected { uri, text, languageId? } params")
    }
    const languageId = typeof params.languageId === "string" ? params.languageId : "tolk"
    server.addSourceFileForLanguage(languageId, String(params.uri), String(params.text))
    return null
  }),
)

connection.onRequest(SET_WORKSPACE_CONFIG_REQUEST, async params =>
  withLanguageServer(SET_WORKSPACE_CONFIG_REQUEST, server => {
    if (!isRecord(params)) {
      throw new Error("expected { languageId, rootUri, manifestUri?, text } params")
    }
    const languageId = requiredString(params.languageId, "languageId")
    const rootUri = requiredString(params.rootUri, "rootUri")
    const manifestUri = typeof params.manifestUri === "string" ? params.manifestUri : ""
    const text = requiredString(params.text, "text")
    server.setWorkspaceConfigForLanguage(languageId, rootUri, manifestUri, text)
    return null
  }),
)

documents.listen(connection)
connection.listen()

async function withLanguageServer<T>(
  operation: string,
  action: (server: TonLanguageServer) => T | Promise<T>,
): Promise<T> {
  try {
    return await action(await getLanguageServer())
  } catch (error) {
    throw new Error(`${operation} failed: ${errorText(error)}`)
  }
}

function errorText(error: unknown): string {
  if (error instanceof Error) {
    return error.message
  }
  if (typeof error === "string") {
    return error
  }
  if (isRecord(error)) {
    for (const key of ["message", "error", "reason"]) {
      const value = error[key]
      if (typeof value === "string" && value.length > 0) {
        return value
      }
    }
    try {
      return JSON.stringify(error)
    } catch {
      return "unknown error"
    }
  }
  return String(error)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null
}

function requiredString(value: unknown, name: string): string {
  if (typeof value !== "string") {
    throw new Error(`${name} must be a string`)
  }
  return value
}

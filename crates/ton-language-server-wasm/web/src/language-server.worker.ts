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
  await getLanguageServer()
  return {
    capabilities: {
      definitionProvider: true,
      hoverProvider: true,
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
  const server = await getLanguageServer()
  server.openDocument(
    event.document.uri,
    event.document.languageId,
    event.document.version,
    event.document.getText(),
  )
})

documents.onDidChangeContent(async event => {
  const server = await getLanguageServer()
  server.changeDocument(event.document.uri, event.document.version, event.document.getText())
})

connection.onDefinition(async params => {
  const server = await getLanguageServer()
  return server.definition(params.textDocument.uri, params.position.line, params.position.character)
})

connection.onHover(async params => {
  const server = await getLanguageServer()
  return server.hover(params.textDocument.uri, params.position.line, params.position.character)
})

connection.onCodeLens(async params => {
  const server = await getLanguageServer()
  return server.codeLens(params.textDocument.uri)
})

connection.onFoldingRanges(async params => {
  const server = await getLanguageServer()
  return server.foldingRanges(params.textDocument.uri)
})

connection.onExecuteCommand(async params => {
  if (params.command === STACK_EFFECT_CODE_LENS_COMMAND) {
    return null
  }
  throw new Error(`Unsupported command: ${params.command}`)
})

connection.onRequest(LOG_LEVEL_REQUEST, async level => {
  const server = await getLanguageServer()
  server.setLogLevel(String(level))
  return server.logs()
})

connection.onRequest(LOGS_REQUEST, async () => {
  const server = await getLanguageServer()
  return server.logs()
})

connection.onRequest(CLEAR_LOGS_REQUEST, async () => {
  const server = await getLanguageServer()
  server.clearLogs()
  return server.logs()
})

connection.onRequest(PROFILE_REQUEST, async () => {
  const server = await getLanguageServer()
  return server.profileSummary()
})

documents.listen(connection)
connection.listen()

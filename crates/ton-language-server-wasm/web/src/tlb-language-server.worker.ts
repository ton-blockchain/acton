import {
  BrowserMessageReader,
  BrowserMessageWriter,
  createConnection,
  TextDocuments,
  TextDocumentSyncKind,
  type InitializeResult,
} from "vscode-languageserver/browser"
import {TextDocument} from "vscode-languageserver-textdocument"

import init, {TlbLanguageServer} from "./wasm/ton_language_server_wasm"

const workerSelf = globalThis as unknown as DedicatedWorkerGlobalScope
const connection = createConnection(
  new BrowserMessageReader(workerSelf),
  new BrowserMessageWriter(workerSelf),
)
const documents = new TextDocuments(TextDocument)

let languageServer: TlbLanguageServer | undefined

const serverReady = init().then(() => {
  languageServer = new TlbLanguageServer()
})

const getLanguageServer = async () => {
  await serverReady
  if (!languageServer) {
    throw new Error("TL-B language server was not initialized")
  }
  return languageServer
}

connection.onInitialize(async (): Promise<InitializeResult> => {
  await getLanguageServer()
  return {
    capabilities: {
      definitionProvider: true,
      textDocumentSync: TextDocumentSyncKind.Full,
    },
  }
})

documents.onDidOpen(async event => {
  const server = await getLanguageServer()
  server.openDocument(event.document.uri, event.document.version, event.document.getText())
})

documents.onDidChangeContent(async event => {
  const server = await getLanguageServer()
  server.changeDocument(event.document.uri, event.document.version, event.document.getText())
})

connection.onDefinition(async params => {
  const server = await getLanguageServer()
  return server.definition(params.textDocument.uri, params.position.line, params.position.character)
})

documents.listen(connection)
connection.listen()

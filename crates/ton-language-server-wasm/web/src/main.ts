import * as vscode from "vscode"
import {EditorApp, type EditorAppConfig} from "monaco-languageclient/editorApp"
import {LanguageClientWrapper} from "monaco-languageclient/lcwrapper"
import {
  MonacoVscodeApiWrapper,
  type MonacoVscodeApiConfig,
} from "monaco-languageclient/vscodeApiWrapper"
import {useWorkerFactory, Worker as MonacoWorker} from "monaco-languageclient/workerFactory"
import editorWorkerUrl from "@codingame/monaco-vscode-editor-api/esm/vs/editor/editor.worker.js?url"

import TlbLanguageServerWorker from "./tlb-language-server.worker?worker"
import {TLB_LANGUAGE_ID, tlbLanguageExtension, tlbMonarchLanguage} from "./tlb-language"
import "./style.css"

const workspaceUri = vscode.Uri.file("/workspace")
const documentUri = vscode.Uri.file("/workspace/main.tlb")
const initialSourceLines = [
  "foo$0 a:# = CommonMsgInfo;",
  "bar$1 b:# = CommonMsgInfo;",
  ...Array.from(
    {length: 800},
    (_, index) => `entry_${index}$0 value:# ref:CommonMsgInfo = Entry${index};`,
  ),
  "baz$2 x:CommonMsgInfo = Wrap;",
]
const initialSource = initialSourceLines.join("\n")
const referenceLine = initialSourceLines.length - 1
const referenceCharacter = initialSourceLines[referenceLine].indexOf("CommonMsgInfo")

type PlainDefinition = {
  readonly uri: string
  readonly range: {
    readonly start: {readonly line: number; readonly character: number}
    readonly end: {readonly line: number; readonly character: number}
  }
}

type SmokeApi = {
  definitionAtReference: () => Promise<PlainDefinition[]>
  editorText: () => string
  languageId: () => string | undefined
}

const smokeGlobal = globalThis as typeof globalThis & {__tlbSmoke?: SmokeApi}
const statusElement = document.getElementById("status")
const editorElement = document.getElementById("monaco-editor-root")

if (!statusElement || !editorElement) {
  throw new Error("TL-B smoke page root elements are missing")
}

const setStatus = (state: "starting" | "ready" | "error", text: string) => {
  statusElement.dataset.state = state
  statusElement.textContent = text
}

const configureMonacoWorkers = () => {
  useWorkerFactory({
    workerLoaders: {
      editorWorkerService: () => new MonacoWorker(editorWorkerUrl, {type: "module"}),
    },
  })
}

const toPlainDefinition = (location: vscode.Location): PlainDefinition => ({
  uri: location.uri.toString(),
  range: {
    start: {
      line: location.range.start.line,
      character: location.range.start.character,
    },
    end: {
      line: location.range.end.line,
      character: location.range.end.character,
    },
  },
})

const start = async () => {
  const vscodeApiConfig: MonacoVscodeApiConfig = {
    $type: "classic",
    viewsConfig: {
      $type: "EditorService",
    },
    userConfiguration: {
      json: JSON.stringify({
        "editor.fontFamily": 'ui-monospace, "SF Mono", Consolas, "Liberation Mono", monospace',
        "editor.fontSize": 13,
        "editor.lineHeight": 20,
        "editor.minimap.enabled": false,
        "editor.wordBasedSuggestions": "off",
        "workbench.colorTheme": "Default Dark Modern",
      }),
    },
    monacoWorkerFactory: configureMonacoWorkers,
    advanced: {
      loadExtensionServices: false,
      loadThemes: false,
    },
  }

  const apiWrapper = new MonacoVscodeApiWrapper(vscodeApiConfig)
  await apiWrapper.start()

  const worker = new TlbLanguageServerWorker()
  const languageClient = new LanguageClientWrapper({
    languageId: TLB_LANGUAGE_ID,
    connection: {
      options: {
        $type: "WorkerDirect",
        worker,
      },
    },
    clientOptions: {
      documentSelector: [{language: TLB_LANGUAGE_ID, scheme: "file"}],
      workspaceFolder: {
        index: 0,
        name: "workspace",
        uri: workspaceUri,
      },
    },
    disposeWorker: true,
  })
  await languageClient.start()

  const editorConfig: EditorAppConfig = {
    codeResources: {
      modified: {
        text: initialSource,
        uri: documentUri.toString(),
        enforceLanguageId: TLB_LANGUAGE_ID,
      },
    },
    languageDef: {
      languageExtensionConfig: tlbLanguageExtension,
      monarchLanguage: tlbMonarchLanguage,
    },
    editorOptions: {
      "semanticHighlighting.enabled": false,
      automaticLayout: true,
      fixedOverflowWidgets: true,
      glyphMargin: false,
      lineDecorationsWidth: 8,
      lineNumbersMinChars: 3,
      padding: {top: 8, bottom: 8},
      renderLineHighlight: "line",
      scrollBeyondLastLine: false,
    },
  }
  const editorApp = new EditorApp(editorConfig)
  await editorApp.start(editorElement)
  const editor = editorApp.getEditor()
  editor?.setPosition({lineNumber: referenceLine + 1, column: referenceCharacter + 1})
  editor?.focus()

  smokeGlobal.__tlbSmoke = {
    async definitionAtReference() {
      const result = await vscode.commands.executeCommand<vscode.Location[]>(
        "vscode.executeDefinitionProvider",
        documentUri,
        new vscode.Position(referenceLine, referenceCharacter),
      )
      return (result ?? []).map(toPlainDefinition)
    },
    editorText() {
      return editor?.getModel()?.getValue() ?? ""
    },
    languageId() {
      return editor?.getModel()?.getLanguageId()
    },
  }

  const definitions = await smokeGlobal.__tlbSmoke.definitionAtReference()
  setStatus("ready", `ready: ${definitions.length} definitions`)
}

start().catch(error => {
  console.error(error)
  setStatus("error", error instanceof Error ? error.message : String(error))
})

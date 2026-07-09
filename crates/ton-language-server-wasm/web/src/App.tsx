import * as monaco from "@codingame/monaco-vscode-editor-api"
import * as vscode from "vscode"
import {useCallback, useEffect, useRef, useState, type ChangeEvent} from "react"
import {EditorApp, type EditorAppConfig} from "monaco-languageclient/editorApp"
import {LanguageClientWrapper} from "monaco-languageclient/lcwrapper"
import {
  MonacoVscodeApiWrapper,
  type MonacoVscodeApiConfig,
} from "monaco-languageclient/vscodeApiWrapper"
import {
  Worker as MonacoWorker,
  useWorkerFactory as configureWorkerFactory,
} from "monaco-languageclient/workerFactory"
import editorWorkerUrl from "@codingame/monaco-vscode-editor-api/esm/vs/editor/editor.worker.js?url"

import LanguageServerWorker from "./language-server.worker?worker"
import {
  defaultLanguageId,
  isSupportedLanguage,
  languageSupportById,
  languageSupports,
  normalizeLanguage,
  type SupportedLanguage,
  TOLK_LANGUAGE_ID,
} from "./languages"
import {actonTomlLanguageSupport, defaultActonTomlSource} from "./languages/acton-toml"

type LogLevelName = "off" | "error" | "warn" | "info" | "debug" | "trace"

type PlainHover = {
  readonly contents: readonly string[]
  readonly range: {
    readonly start: {readonly line: number; readonly character: number}
    readonly end: {readonly line: number; readonly character: number}
  } | null
}

type PlainCodeLens = {
  readonly title: string | null
  readonly command: string | null
  readonly range: {
    readonly start: {readonly line: number; readonly character: number}
    readonly end: {readonly line: number; readonly character: number}
  }
}

type PlainFoldingRange = {
  readonly start: number
  readonly end: number
  readonly kind: string | null
}

type PlainLocation = {
  readonly uri: string
  readonly range: {
    readonly start: {readonly line: number; readonly character: number}
    readonly end: {readonly line: number; readonly character: number}
  }
}

type PersistedState = {
  selectedLanguage: SupportedLanguage
  logLevel: LogLevelName
  logsVisible: boolean
  profileVisible: boolean
  actonTomlVisible: boolean
  actonToml: string
  files: Record<SupportedLanguage, string>
}

type SmokeApi = {
  hoverAtInstruction: () => Promise<PlainHover[]>
  definitionAt: (line: number, character: number) => Promise<PlainLocation[]>
  codeLenses: () => Promise<PlainCodeLens[]>
  foldingRanges: () => Promise<PlainFoldingRange[]>
  logs: () => Promise<string>
  profile: () => Promise<string>
  sidePanelText: () => string
  logsPanelText: () => string
  profilePanelText: () => string
  persistedState: () => PersistedState | null
  selectedLanguage: () => SupportedLanguage
  setLanguage: (languageId: SupportedLanguage) => Promise<void>
  setEditorText: (text: string) => void
  setLogLevel: (level: LogLevelName) => Promise<void>
  setLogsVisible: (visible: boolean) => void
  setProfileVisible: (visible: boolean) => void
  setActonTomlVisible: (visible: boolean) => void
  setActonTomlText: (text: string) => void
  editorText: () => string
  actonTomlText: () => string
  languageId: () => string | undefined
}

const workspaceUri = vscode.Uri.file("/workspace")
const actonTomlUri = vscode.Uri.file("/workspace/Acton.toml")
const storageKey = "ton-language-server-web-state:v1"
const logLevelRequest = "ton/setLogLevel"
const logsRequest = "ton/logs"
const clearLogsRequest = "ton/clearLogs"
const profileRequest = "ton/profile"
const setWorkspaceConfigRequest = "ton/setWorkspaceConfig"

const logLevels: readonly LogLevelName[] = ["off", "error", "warn", "info", "debug", "trace"]

const smokeGlobal = globalThis as typeof globalThis & {__tonLsSmoke?: SmokeApi}

let languagesRegistered = false

export function App() {
  const [persisted, setPersisted] = useState(loadState)
  const [status, setStatus] = useState<{state: "starting" | "ready" | "error"; text: string}>({
    state: "starting",
    text: "starting",
  })

  const persistedRef = useRef(persisted)
  const currentLanguageRef = useRef(persisted.selectedLanguage)
  const editorRootRef = useRef<HTMLDivElement | null>(null)
  const actonConfigRootRef = useRef<HTMLDivElement | null>(null)
  const profileRootRef = useRef<HTMLDivElement | null>(null)
  const logsRootRef = useRef<HTMLDivElement | null>(null)
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | undefined>(undefined)
  const actonConfigEditorRef = useRef<monaco.editor.IStandaloneCodeEditor | undefined>(undefined)
  const profileEditorRef = useRef<monaco.editor.IStandaloneCodeEditor | undefined>(undefined)
  const logsEditorRef = useRef<monaco.editor.IStandaloneCodeEditor | undefined>(undefined)
  const actonConfigModelRef = useRef<monaco.editor.ITextModel | undefined>(undefined)
  const profileModelRef = useRef<monaco.editor.ITextModel | undefined>(undefined)
  const logsModelRef = useRef<monaco.editor.ITextModel | undefined>(undefined)
  const profileRefreshTimerRef = useRef<ReturnType<typeof globalThis.setInterval> | undefined>(
    undefined,
  )
  const logsRefreshTimerRef = useRef<ReturnType<typeof globalThis.setInterval> | undefined>(
    undefined,
  )
  const switchingLanguageRef = useRef(false)
  const switchLanguageRef = useRef<(languageId: SupportedLanguage) => Promise<void>>(async () => {})
  const setLogLevelRef = useRef<(level: LogLevelName) => Promise<void>>(async () => {})
  const setLogsVisibleRef = useRef<(visible: boolean) => Promise<void>>(async () => {})
  const setProfileVisibleRef = useRef<(visible: boolean) => Promise<void>>(async () => {})
  const setActonTomlVisibleRef = useRef<(visible: boolean) => Promise<void>>(async () => {})
  const clearLogsRef = useRef<() => Promise<void>>(async () => {})

  const persist = useCallback((next: PersistedState, updateReact = true) => {
    persistedRef.current = next
    localStorage.setItem(storageKey, JSON.stringify(next))
    if (updateReact) {
      setPersisted(next)
    }
  }, [])

  const saveCurrentFile = useCallback(
    (fallbackLanguage: SupportedLanguage = currentLanguageRef.current) => {
      const model = editorRef.current?.getModel()
      const text = model?.getValue()
      if (text === undefined) {
        return
      }
      const languageId = languageFromModel(model, fallbackLanguage)
      persist(
        {
          ...persistedRef.current,
          files: {
            ...persistedRef.current.files,
            [languageId]: text,
          },
        },
        false,
      )
    },
    [persist],
  )

  useEffect(() => {
    let disposed = false
    const disposables: monaco.IDisposable[] = []
    const cleanupCallbacks: Array<() => void> = []

    const configureMonacoWorkers = () => {
      configureWorkerFactory({
        workerLoaders: {
          editorWorkerService: () => new MonacoWorker(editorWorkerUrl, {type: "module"}),
        },
      })
    }

    const registerLanguages = () => {
      if (languagesRegistered) {
        return
      }
      for (const language of languageSupports) {
        monaco.languages.register(language.extensionPoint)
        monaco.languages.setMonarchTokensProvider(language.id, language.monarchLanguage)
      }
      monaco.languages.register(actonTomlLanguageSupport.extensionPoint)
      monaco.languages.setMonarchTokensProvider(
        actonTomlLanguageSupport.id,
        actonTomlLanguageSupport.monarchLanguage,
      )
      languagesRegistered = true
    }

    const updateLogsEditor = (logs: string) => {
      const logsModel = logsModelRef.current
      if (!logsModel) {
        return
      }
      const nextValue = logs || "No log entries"
      if (logsModel.getValue() === nextValue) {
        return
      }
      logsModel.setValue(nextValue)
      logsEditorRef.current?.revealLine(logsModel.getLineCount())
    }

    const updateProfileEditor = (profile: string) => {
      const profileModel = profileModelRef.current
      if (!profileModel) {
        return
      }
      const nextValue = profile || "No profiling data"
      if (profileModel.getValue() === nextValue) {
        return
      }
      profileModel.setValue(nextValue)
    }

    const start = async () => {
      const editorRoot = editorRootRef.current
      const actonConfigRoot = actonConfigRootRef.current
      const profileRoot = profileRootRef.current
      const logsRoot = logsRootRef.current
      if (!editorRoot || !actonConfigRoot || !profileRoot || !logsRoot) {
        throw new Error("TON LS editor roots are missing")
      }

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
            "editor.codeLens": true,
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
      registerLanguages()

      const worker = new LanguageServerWorker()
      const languageClient = new LanguageClientWrapper({
        languageId: "ton-language-server",
        connection: {
          options: {
            $type: "WorkerDirect",
            worker,
          },
        },
        clientOptions: {
          documentSelector: languageSupports.map(language => ({
            language: language.id,
            scheme: "file",
          })),
          workspaceFolder: {
            index: 0,
            name: "workspace",
            uri: workspaceUri,
          },
        },
        disposeWorker: true,
      })
      await languageClient.start()

      const sendRequest = async <T,>(method: string, params?: unknown): Promise<T> => {
        const client = languageClient.getLanguageClient()
        if (!client) {
          throw new Error("language client is not available")
        }
        return client.sendRequest(method, params)
      }

      const applyWorkspaceConfig = async (text = persistedRef.current.actonToml) => {
        await sendRequest<null>(setWorkspaceConfigRequest, {
          languageId: TOLK_LANGUAGE_ID,
          rootUri: workspaceUri.toString(),
          manifestUri: actonTomlUri.toString(),
          text,
        })
      }

      const applyWorkspaceConfigIfAvailable = async (text = persistedRef.current.actonToml) => {
        try {
          await applyWorkspaceConfig(text)
        } catch (error) {
          if (!errorText(error).includes("unsupported language 'tolk'")) {
            throw error
          }
        }
      }

      const refreshLogs = async () => {
        if (!persistedRef.current.logsVisible) {
          return
        }
        updateLogsEditor(await sendRequest<string>(logsRequest))
      }

      const refreshProfile = async () => {
        if (!persistedRef.current.profileVisible) {
          return
        }
        updateProfileEditor(await sendRequest<string>(profileRequest))
      }

      const applyLogLevel = async (level: LogLevelName) => {
        const logs = await sendRequest<string>(logLevelRequest, level)
        if (persistedRef.current.logsVisible) {
          updateLogsEditor(logs)
        }
      }

      const applyLogsVisibility = async (visible: boolean) => {
        logsEditorRef.current?.layout()
        if (logsRefreshTimerRef.current !== undefined) {
          globalThis.clearInterval(logsRefreshTimerRef.current)
          logsRefreshTimerRef.current = undefined
        }
        if (visible) {
          await refreshLogs()
          logsRefreshTimerRef.current = globalThis.setInterval(() => {
            void refreshLogs()
          }, 750)
        }
      }

      const applyProfileVisibility = async (visible: boolean) => {
        profileEditorRef.current?.layout()
        if (profileRefreshTimerRef.current !== undefined) {
          globalThis.clearInterval(profileRefreshTimerRef.current)
          profileRefreshTimerRef.current = undefined
        }
        if (visible) {
          await refreshProfile()
          profileRefreshTimerRef.current = globalThis.setInterval(() => {
            void refreshProfile()
          }, 500)
        }
      }

      const applyActonTomlVisibility = async () => {
        editorRef.current?.layout()
        actonConfigEditorRef.current?.layout()
      }

      const clearLogs = async () => {
        const logs = await sendRequest<string>(clearLogsRequest)
        if (persistedRef.current.logsVisible) {
          updateLogsEditor(logs)
        }
      }

      await applyWorkspaceConfigIfAvailable()

      const editorConfig: EditorAppConfig = {
        codeResources: {
          modified: {
            text: sourceFor(currentLanguageRef.current, persistedRef.current),
            uri: documentUriFor(currentLanguageRef.current).toString(),
            enforceLanguageId: currentLanguageRef.current,
          },
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
      await editorApp.start(editorRoot)
      editorRef.current = editorApp.getEditor()

      actonConfigModelRef.current = monaco.editor.createModel(
        persistedRef.current.actonToml,
        actonTomlLanguageSupport.id,
        actonTomlUri,
      )
      actonConfigEditorRef.current = monaco.editor.create(actonConfigRoot, {
        automaticLayout: true,
        fixedOverflowWidgets: true,
        glyphMargin: false,
        lineDecorationsWidth: 8,
        lineNumbersMinChars: 3,
        minimap: {enabled: false},
        model: actonConfigModelRef.current,
        padding: {top: 8, bottom: 8},
        renderLineHighlight: "line",
        scrollBeyondLastLine: false,
      })

      profileModelRef.current = monaco.editor.createModel("No profiling data", "plaintext")
      profileEditorRef.current = monaco.editor.create(profileRoot, {
        automaticLayout: true,
        fixedOverflowWidgets: true,
        glyphMargin: false,
        lineDecorationsWidth: 8,
        lineNumbersMinChars: 4,
        minimap: {enabled: false},
        model: profileModelRef.current,
        padding: {top: 8, bottom: 8},
        readOnly: true,
        renderLineHighlight: "none",
        scrollBeyondLastLine: false,
        wordWrap: "on",
      })

      logsModelRef.current = monaco.editor.createModel("No log entries", "plaintext")
      logsEditorRef.current = monaco.editor.create(logsRoot, {
        automaticLayout: true,
        fixedOverflowWidgets: true,
        glyphMargin: false,
        lineDecorationsWidth: 8,
        lineNumbersMinChars: 4,
        minimap: {enabled: false},
        model: logsModelRef.current,
        padding: {top: 8, bottom: 8},
        readOnly: true,
        renderLineHighlight: "none",
        scrollBeyondLastLine: false,
        wordWrap: "on",
      })

      editorRef.current?.setPosition({lineNumber: 1, column: 1})
      editorRef.current?.focus()
      const editor = editorRef.current
      if (editor) {
        disposables.push(
          editor.onDidChangeModelContent(() => {
            if (switchingLanguageRef.current) {
              return
            }
            saveCurrentFile()
            setStatus({
              state: "ready",
              text: `${languageLabel(currentLanguageRef.current)} saved locally`,
            })
            void refreshLogs()
            void refreshProfile()
          }),
        )
      }
      const actonConfigModel = actonConfigModelRef.current
      if (actonConfigModel) {
        disposables.push(
          actonConfigModel.onDidChangeContent(() => {
            const actonToml = actonConfigModel.getValue()
            persist(
              {
                ...persistedRef.current,
                actonToml,
              },
              false,
            )
            void applyWorkspaceConfigIfAvailable(actonToml)
              .then(async () => {
                setStatus({state: "ready", text: "Acton.toml saved locally"})
                await refreshLogs()
                await refreshProfile()
              })
              .catch((error: unknown) => {
                console.error(error)
                setStatus({state: "error", text: errorText(error)})
              })
          }),
        )
      }

      const switchLanguage = async (languageId: SupportedLanguage) => {
        if (languageId === currentLanguageRef.current) {
          return
        }
        saveCurrentFile(currentLanguageRef.current)
        currentLanguageRef.current = languageId
        persist({
          ...persistedRef.current,
          selectedLanguage: languageId,
        })
        if (languageId === TOLK_LANGUAGE_ID) {
          await applyWorkspaceConfigIfAvailable()
        }
        switchingLanguageRef.current = true
        try {
          await editorApp.updateCodeResources({
            modified: {
              text: sourceFor(languageId, persistedRef.current),
              uri: documentUriFor(languageId).toString(),
              enforceLanguageId: languageId,
            },
          })
        } finally {
          switchingLanguageRef.current = false
        }
        editorRef.current?.setPosition({lineNumber: 1, column: 1})
        setStatus({state: "ready", text: `${languageLabel(languageId)} saved locally`})
        await refreshLogs()
        await refreshProfile()
      }

      switchLanguageRef.current = switchLanguage
      setLogLevelRef.current = applyLogLevel
      setLogsVisibleRef.current = applyLogsVisibility
      setProfileVisibleRef.current = applyProfileVisibility
      setActonTomlVisibleRef.current = applyActonTomlVisibility
      clearLogsRef.current = clearLogs

      const flushCurrentFile = () => saveCurrentFile(currentLanguageRef.current)
      window.addEventListener("beforeunload", flushCurrentFile)
      window.addEventListener("pagehide", flushCurrentFile)
      document.addEventListener("visibilitychange", flushCurrentFile)
      cleanupCallbacks.push(() => {
        window.removeEventListener("beforeunload", flushCurrentFile)
        window.removeEventListener("pagehide", flushCurrentFile)
        document.removeEventListener("visibilitychange", flushCurrentFile)
      })

      window.addEventListener("resize", layoutEditors)
      cleanupCallbacks.push(() => window.removeEventListener("resize", layoutEditors))
      await applyLogLevel(persistedRef.current.logLevel)
      await applyActonTomlVisibility()
      await applyLogsVisibility(persistedRef.current.logsVisible)
      await applyProfileVisibility(persistedRef.current.profileVisible)

      smokeGlobal.__tonLsSmoke = {
        async hoverAtInstruction() {
          const result = await vscode.commands.executeCommand<vscode.Hover[]>(
            "vscode.executeHoverProvider",
            documentUriFor(currentLanguageRef.current),
            new vscode.Position(0, 0),
          )
          return (result ?? []).map(toPlainHover)
        },
        async definitionAt(line: number, character: number) {
          const result = await vscode.commands.executeCommand<vscode.Location[]>(
            "vscode.executeDefinitionProvider",
            documentUriFor(currentLanguageRef.current),
            new vscode.Position(line, character),
          )
          return (result ?? []).map(toPlainLocation)
        },
        async codeLenses() {
          const result = await vscode.commands.executeCommand<vscode.CodeLens[]>(
            "vscode.executeCodeLensProvider",
            documentUriFor(currentLanguageRef.current),
          )
          return (result ?? []).map(toPlainCodeLens)
        },
        async foldingRanges() {
          const result = await vscode.commands.executeCommand<vscode.FoldingRange[]>(
            "vscode.executeFoldingRangeProvider",
            documentUriFor(currentLanguageRef.current),
          )
          return (result ?? []).map(toPlainFoldingRange)
        },
        async logs() {
          return sendRequest<string>(logsRequest)
        },
        async profile() {
          return sendRequest<string>(profileRequest)
        },
        sidePanelText() {
          return persistedRef.current.profileVisible
            ? (profileModelRef.current?.getValue() ?? "")
            : (logsModelRef.current?.getValue() ?? "")
        },
        logsPanelText() {
          return logsModelRef.current?.getValue() ?? ""
        },
        profilePanelText() {
          return profileModelRef.current?.getValue() ?? ""
        },
        persistedState() {
          const raw = localStorage.getItem(storageKey)
          return raw ? (JSON.parse(raw) as PersistedState) : null
        },
        selectedLanguage() {
          return currentLanguageRef.current
        },
        async setLanguage(languageId: SupportedLanguage) {
          await switchLanguage(normalizeLanguage(languageId))
        },
        setEditorText(text: string) {
          editorRef.current?.getModel()?.setValue(text)
          saveCurrentFile(currentLanguageRef.current)
        },
        async setLogLevel(level: LogLevelName) {
          const normalized = normalizeLogLevel(level)
          persist({...persistedRef.current, logLevel: normalized})
          await applyLogLevel(normalized)
        },
        setLogsVisible(visible: boolean) {
          persist({...persistedRef.current, logsVisible: visible})
          void applyLogsVisibility(visible)
        },
        setProfileVisible(visible: boolean) {
          persist({...persistedRef.current, profileVisible: visible})
          void applyProfileVisibility(visible)
        },
        setActonTomlVisible(visible: boolean) {
          persist({...persistedRef.current, actonTomlVisible: visible})
          void applyActonTomlVisibility()
        },
        setActonTomlText(text: string) {
          actonConfigModelRef.current?.setValue(text)
        },
        editorText() {
          return editorRef.current?.getModel()?.getValue() ?? ""
        },
        actonTomlText() {
          return actonConfigModelRef.current?.getValue() ?? ""
        },
        languageId() {
          return editorRef.current?.getModel()?.getLanguageId()
        },
      }

      const hovers = await smokeGlobal.__tonLsSmoke.hoverAtInstruction()
      if (!disposed) {
        setStatus({
          state: "ready",
          text: `${languageLabel(currentLanguageRef.current)} saved locally, ${hovers.length} hovers`,
        })
      }

      function layoutEditors() {
        editorRef.current?.layout()
        actonConfigEditorRef.current?.layout()
        profileEditorRef.current?.layout()
        logsEditorRef.current?.layout()
      }
    }

    start().catch((error: unknown) => {
      console.error(error)
      if (!disposed) {
        setStatus({state: "error", text: errorText(error)})
      }
    })

    return () => {
      disposed = true
      if (logsRefreshTimerRef.current !== undefined) {
        globalThis.clearInterval(logsRefreshTimerRef.current)
      }
      if (profileRefreshTimerRef.current !== undefined) {
        globalThis.clearInterval(profileRefreshTimerRef.current)
      }
      for (const disposable of disposables) {
        disposable.dispose()
      }
      for (const cleanup of cleanupCallbacks) {
        cleanup()
      }
      actonConfigEditorRef.current?.dispose()
      actonConfigModelRef.current?.dispose()
      profileEditorRef.current?.dispose()
      profileModelRef.current?.dispose()
      logsEditorRef.current?.dispose()
      logsModelRef.current?.dispose()
      smokeGlobal.__tonLsSmoke = undefined
    }
  }, [persist, saveCurrentFile])

  const handleLanguageChange = (event: ChangeEvent<HTMLSelectElement>) => {
    const languageId = normalizeLanguage(event.currentTarget.value)
    persist({...persistedRef.current, selectedLanguage: languageId})
    void switchLanguageRef.current(languageId).catch((error: unknown) => {
      console.error(error)
      setStatus({state: "error", text: errorText(error)})
    })
  }

  const handleLogLevelChange = (event: ChangeEvent<HTMLSelectElement>) => {
    const logLevel = normalizeLogLevel(event.currentTarget.value)
    persist({...persistedRef.current, logLevel})
    void setLogLevelRef.current(logLevel).catch((error: unknown) => {
      console.error(error)
      setStatus({state: "error", text: errorText(error)})
    })
  }

  const handleLogsVisibleChange = (event: ChangeEvent<HTMLInputElement>) => {
    const logsVisible = event.currentTarget.checked
    persist({...persistedRef.current, logsVisible})
    void setLogsVisibleRef.current(logsVisible).catch((error: unknown) => {
      console.error(error)
      setStatus({state: "error", text: errorText(error)})
    })
  }

  const handleProfileVisibleChange = (event: ChangeEvent<HTMLInputElement>) => {
    const profileVisible = event.currentTarget.checked
    persist({...persistedRef.current, profileVisible})
    void setProfileVisibleRef.current(profileVisible).catch((error: unknown) => {
      console.error(error)
      setStatus({state: "error", text: errorText(error)})
    })
  }

  const handleActonTomlVisibleChange = (event: ChangeEvent<HTMLInputElement>) => {
    const actonTomlVisible = event.currentTarget.checked
    persist({...persistedRef.current, actonTomlVisible})
    void setActonTomlVisibleRef.current(actonTomlVisible).catch((error: unknown) => {
      console.error(error)
      setStatus({state: "error", text: errorText(error)})
    })
  }

  const handleClearLogs = () => {
    void clearLogsRef.current().catch((error: unknown) => {
      console.error(error)
      setStatus({state: "error", text: errorText(error)})
    })
  }

  return (
    <div
      id="app-shell"
      data-show-logs={String(persisted.logsVisible)}
      data-show-profile={String(persisted.profileVisible)}
      data-show-acton-config={String(persisted.actonTomlVisible)}
      data-show-panel={String(persisted.logsVisible || persisted.profileVisible)}
    >
      <div id="toolbar">
        <label className="field">
          <span>Language</span>
          <select
            id="language-select"
            value={persisted.selectedLanguage}
            onChange={handleLanguageChange}
          >
            {languageSupports.map(language => (
              <option key={language.id} value={language.id}>
                {language.label}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Logs</span>
          <select id="log-level-select" value={persisted.logLevel} onChange={handleLogLevelChange}>
            {logLevels.map(level => (
              <option key={level} value={level}>
                {level[0].toUpperCase() + level.slice(1)}
              </option>
            ))}
          </select>
        </label>
        <label className="check-field">
          <input
            id="logs-toggle"
            type="checkbox"
            checked={persisted.logsVisible}
            onChange={handleLogsVisibleChange}
          />
          <span>Show logs</span>
        </label>
        <button type="button" className="toolbar-button" onClick={handleClearLogs}>
          Clear
        </button>
        <label className="check-field">
          <input
            id="acton-config-toggle"
            type="checkbox"
            checked={persisted.actonTomlVisible}
            onChange={handleActonTomlVisibleChange}
          />
          <span>Acton.toml</span>
        </label>
        <label className="check-field">
          <input
            id="profile-toggle"
            type="checkbox"
            checked={persisted.profileVisible}
            onChange={handleProfileVisibleChange}
          />
          <span>Perf</span>
        </label>
        <div id="status" aria-live="polite" data-state={status.state}>
          {status.text}
        </div>
      </div>
      <div id="editor-shell">
        <div id="main-panel">
          <div id="monaco-editor-root" ref={editorRootRef} />
          <div id="acton-config-editor-root" ref={actonConfigRootRef} />
        </div>
        <div id="side-panel">
          <div id="profile-editor-root" ref={profileRootRef} />
          <div id="logs-editor-root" ref={logsRootRef} />
        </div>
      </div>
    </div>
  )
}

function documentUriFor(languageId: SupportedLanguage) {
  return vscode.Uri.file(`/workspace/main.${languageSupportById[languageId].fileExtension}`)
}

function sourceFor(languageId: SupportedLanguage, state: PersistedState) {
  return state.files[languageId] ?? languageSupportById[languageId].defaultSource
}

function languageFromModel(
  model: monaco.editor.ITextModel | null | undefined,
  fallback: SupportedLanguage,
): SupportedLanguage {
  const languageId = model?.getLanguageId()
  return isSupportedLanguage(languageId) ? languageId : fallback
}

function normalizeLogLevel(value: unknown): LogLevelName {
  if (
    value === "off" ||
    value === "error" ||
    value === "warn" ||
    value === "info" ||
    value === "debug" ||
    value === "trace"
  ) {
    return value
  }
  return "info"
}

function errorText(error: unknown) {
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
      return "Unknown error"
    }
  }
  return String(error)
}

function loadState(): PersistedState {
  const fallback: PersistedState = {
    selectedLanguage: defaultLanguageId,
    logLevel: "info",
    logsVisible: false,
    profileVisible: false,
    actonTomlVisible: false,
    actonToml: defaultActonTomlSource,
    files: defaultFiles(),
  }
  const raw = localStorage.getItem(storageKey)
  if (!raw) {
    return fallback
  }
  try {
    const parsed = JSON.parse(raw) as Partial<PersistedState>
    return {
      selectedLanguage: normalizeLanguage(parsed.selectedLanguage),
      logLevel: normalizeLogLevel(parsed.logLevel),
      logsVisible: parsed.logsVisible === true,
      profileVisible: parsed.profileVisible === true,
      actonTomlVisible: parsed.actonTomlVisible === true,
      actonToml: typeof parsed.actonToml === "string" ? parsed.actonToml : defaultActonTomlSource,
      files: readPersistedFiles(parsed.files),
    }
  } catch {
    return fallback
  }
}

function defaultFiles(): Record<SupportedLanguage, string> {
  return Object.fromEntries(
    languageSupports.map(language => [language.id, language.defaultSource]),
  ) as Record<SupportedLanguage, string>
}

function readPersistedFiles(value: unknown): Record<SupportedLanguage, string> {
  const files = isRecord(value) ? value : {}
  return Object.fromEntries(
    languageSupports.map(language => {
      const saved = files[language.id]
      return [language.id, typeof saved === "string" ? saved : language.defaultSource]
    }),
  ) as Record<SupportedLanguage, string>
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null
}

function languageLabel(languageId: SupportedLanguage) {
  return languageSupportById[languageId].label
}

function hoverContentToString(content: vscode.MarkedString | vscode.MarkdownString) {
  if (typeof content === "string") {
    return content
  }
  return content.value
}

function toPlainHover(hover: vscode.Hover): PlainHover {
  return {
    contents: hover.contents.map(hoverContentToString),
    range: hover.range
      ? {
          start: {
            line: hover.range.start.line,
            character: hover.range.start.character,
          },
          end: {
            line: hover.range.end.line,
            character: hover.range.end.character,
          },
        }
      : null,
  }
}

function toPlainCodeLens(lens: vscode.CodeLens): PlainCodeLens {
  return {
    title: lens.command?.title ?? null,
    command: lens.command?.command ?? null,
    range: {
      start: {
        line: lens.range.start.line,
        character: lens.range.start.character,
      },
      end: {
        line: lens.range.end.line,
        character: lens.range.end.character,
      },
    },
  }
}

function toPlainLocation(location: vscode.Location): PlainLocation {
  return {
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
  }
}

function toPlainFoldingRange(range: vscode.FoldingRange): PlainFoldingRange {
  return {
    start: range.start,
    end: range.end,
    kind: range.kind === undefined ? null : String(range.kind),
  }
}

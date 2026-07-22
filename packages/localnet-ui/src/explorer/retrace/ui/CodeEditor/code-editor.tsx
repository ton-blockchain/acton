import type React from "react"
import {memo, useEffect, useRef, useState} from "react"
import {Editor, loader} from "@monaco-editor/react"

import * as monaco from "monaco-editor"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker"

import type {ExitCode} from "../../txTrace/lib/types"

import type {LinesExecutionData} from "../../txTrace/hooks"

import {
  useMonacoSetup,
  initializeMonaco,
  useDecorations,
  useEditorEvents,
  useTasmHoverProvider,
  useCodeLensProvider,
  useTasmInlayProvider,
  useSourceDebugValuesProvider,
  useFolding,
  type SupportedLanguage,
  type HighlightGroup,
  type SourceDebugVariableValue,
  type CodeLensAnnotation,
} from "./hooks"

import styles from "./CodeEditor.module.css"

interface CodeEditorProps {
  /* -------------------------------- Core Editor -------------------------------- */
  /** The source code to display in the editor */
  readonly code: string

  /** Programming language for syntax highlighting. Supports 'tasm', 'func', and 'tolk' */
  readonly language?: SupportedLanguage

  /* -------------------------------- Trace Features -------------------------------- */
  /** Line number to highlight (1-indexed). Used for showing the current execution step */
  readonly highlightLine?: number

  /** Line to show implicit RET marker (placed under previous instruction) */
  readonly implicitRetLine?: number
  /** Custom label for implicit RET inlay hint */
  readonly implicitRetLabel?: string

  /** Execution data for each line including gas costs and execution counts */
  readonly lineExecutionData?: LinesExecutionData

  /** Callback fired when a user ctrl+clicks on a line with gas data */
  readonly onLineClick?: (line: number) => void

  /** Whether to center the editor view on the highlighted line */
  readonly shouldCenter?: boolean

  /** Optional line to center when it differs from the current execution highlight. */
  readonly centerLine?: number

  /** Exit code information to display as code lens above the error line */
  readonly exitCode?: ExitCode

  /** Compiler ABI used to resolve custom thrown error names. */
  readonly compilerAbi?: ContractABI

  /** Explicit code lens annotation to display above a line. */
  readonly codeLensAnnotation?: CodeLensAnnotation

  /** Whether to show instruction documentation in hover tooltips for TASM */
  readonly showInstructionDocs?: boolean

  /* -------------------------------- Godbolt/Source Mapping -------------------------------- */
  /** Groups of lines to highlight with different colors. Used for source map visualization */
  readonly highlightGroups?: readonly HighlightGroup[]

  /** Whether to show the floating tip for the editor */
  readonly needFloatingTip?: boolean

  /** Optional explicit Monaco model path to avoid sharing models between editors */
  readonly modelPath?: string

  /** Source-debug locals used for hover values. */
  readonly sourceDebugVariables?: readonly SourceDebugVariableValue[]
}

// use local instance of monaco
loader.config({monaco})

const monacoGlobal = globalThis as typeof globalThis & {
  MonacoEnvironment?: {
    getWorker: () => Worker
  }
}

monacoGlobal.MonacoEnvironment = {
  getWorker() {
    // basic worker for complex tasks
    return new editorWorker()
  },
}

const CodeEditor: React.FC<CodeEditorProps> = ({
  code,
  highlightLine,
  implicitRetLine,
  implicitRetLabel,
  lineExecutionData,
  onLineClick,
  shouldCenter = true,
  centerLine,
  exitCode,
  compilerAbi,
  codeLensAnnotation,
  language = "tasm",
  highlightGroups = [],
  showInstructionDocs = true,
  needFloatingTip = lineExecutionData && language === "tasm",
  modelPath,
  sourceDebugVariables,
}) => {
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null)
  const hasFoldedInactiveBlocksRef = useRef(false)
  const [editorReady, setEditorReady] = useState(false)
  const modelKey =
    modelPath ?? (language === "func" ? "main.fc" : language === "tolk" ? "main.tolk" : "out.tasm")

  const {monaco, isMac, theme} = useMonacoSetup({language})

  const {isCtrlPressed, hoveredLine} = useEditorEvents({
    monaco,
    editorRef,
    lineExecutionData,
    onLineClick,
    editorReady,
  })

  const {updateDecorations} = useDecorations({
    monaco,
    highlightLine,
    implicitRetLine,
    lineExecutionData,
    highlightGroups,
    isCtrlPressed,
    hoveredLine,
    shouldCenter,
    centerLine,
  })

  useTasmHoverProvider({
    monaco,
    lineExecutionData,
    showInstructionDocs,
    editorReady,
    enabled: language === "tasm",
  })

  useCodeLensProvider({
    monaco,
    editorRef,
    languageId: language,
    exitCode,
    compilerAbi,
    annotation: codeLensAnnotation,
    editorReady,
    enabled: language === "tasm" || codeLensAnnotation !== undefined,
  })

  useTasmInlayProvider({
    monaco,
    implicitRetLine,
    implicitRetLabel,
    editorRef,
    editorReady,
    enabled: language === "tasm",
  })

  useSourceDebugValuesProvider({
    monaco,
    editorRef,
    editorReady,
    languageId: language,
    variables: sourceDebugVariables,
    enabled: sourceDebugVariables !== undefined && sourceDebugVariables.length > 0,
  })

  const {collapseInactiveBlocks} = useFolding({
    monaco,
    editorRef,
    lineExecutionData,
  })

  useEffect(() => {
    hasFoldedInactiveBlocksRef.current = false

    if (!editorRef.current || !lineExecutionData || Object.keys(lineExecutionData).length === 0) {
      return
    }
    try {
      editorRef.current.trigger("unfold", "editor.unfoldAll", {})
    } catch {
      /* ignore */
    }
  }, [code, language, lineExecutionData, modelKey])

  /* -------------------------------- effects ------------------------------ */
  useEffect(() => {
    if (!editorReady || !editorRef.current) return

    const frame = globalThis.requestAnimationFrame(() => {
      const editor = editorRef.current
      if (!editor) {
        return
      }

      editor.layout()
      updateDecorations(editor)

      if (
        !hasFoldedInactiveBlocksRef.current &&
        lineExecutionData &&
        Object.keys(lineExecutionData).length > 0
      ) {
        hasFoldedInactiveBlocksRef.current = true
        collapseInactiveBlocks()
      }
    })

    return () => globalThis.cancelAnimationFrame(frame)
  }, [
    code,
    collapseInactiveBlocks,
    editorReady,
    language,
    lineExecutionData,
    modelKey,
    updateDecorations,
  ])

  /* -------------------------------- render ------------------------------- */
  return (
    <>
      <div className={styles.editorWrapper}>
        <Editor
          className={styles.editor}
          height="100%"
          width="100%"
          language={language}
          theme={theme}
          path={modelKey}
          value={code}
          saveViewState
          keepCurrentModel
          options={{
            minimap: {enabled: false},
            readOnly: true,
            lineNumbers: "on",
            automaticLayout: true,
            scrollBeyondLastLine: false,
            wordWrap: "on",
            fontSize: 13.5,
            tabSize: 4,
            insertSpaces: true,
            detectIndentation: false,
            fontFamily: "JetBrains Mono",
            glyphMargin: false,
            lineDecorationsWidth: 6,
            lineNumbersMinChars: 2,
            renderLineHighlight: "none",
            hideCursorInOverviewRuler: true,
            overviewRulerBorder: false,
            folding: true,
            foldingStrategy: "auto",
            stickyScroll: {enabled: false},
            fixedOverflowWidgets: true,
            scrollbar: {
              useShadows: false,
            },
          }}
          loading={<></>}
          beforeMount={monacoInstance => {
            initializeMonaco(monacoInstance, language)
            monacoInstance.editor.setTheme(theme)
          }}
          onMount={editor => {
            const model = editor.getModel()
            if (monaco && model) {
              model.setEOL(monaco.editor.EndOfLineSequence.LF)
            }

            editorRef.current = editor
            setEditorReady(true)
          }}
        />
      </div>
      {needFloatingTip && (
        <div className={styles.editorHint}>
          <kbd>{isMac ? "⌘" : "Ctrl"}</kbd> + <kbd>Click</kbd> to navigate to trace step
          <span className={styles.hintDivider}>|</span>
          <kbd>←</kbd> <kbd>→</kbd> to step through trace
        </div>
      )}
    </>
  )
}

CodeEditor.displayName = "CodeEditor"

export default memo(CodeEditor)

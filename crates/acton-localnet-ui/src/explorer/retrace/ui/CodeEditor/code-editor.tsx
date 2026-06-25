import React, {memo, useCallback, useEffect, useRef, useState} from "react"
import {Editor, loader} from "@monaco-editor/react"

import * as monaco from "monaco-editor"

import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker"

import type {ExitCode} from "../../txTrace/lib/types"

import type {LinesExecutionData} from "../../txTrace/hooks"

import {useTolkLanguageProviders} from "./hooks/useTolkLanguageProviders"

import {
  useMonacoSetup,
  useDecorations,
  useEditorEvents,
  useTasmHoverProvider,
  useTasmCodeLensProvider,
  useTasmCompletionProvider,
  useTasmInlayProvider,
  useImplicitRetInlayProvider,
  useSourceDebugValuesProvider,
  useFuncLanguageProviders,
  useFolding,
  type SupportedLanguage,
  type HighlightGroup,
  type HighlightRange,
  type SourceDebugVariableValue,
} from "./hooks"

import styles from "./CodeEditor.module.css"

interface CodeEditorProps {
  /* -------------------------------- Core Editor -------------------------------- */
  /** The source code to display in the editor */
  readonly code: string

  /** Programming language for syntax highlighting. Supports 'tasm', 'func', and 'tolk' */
  readonly language?: SupportedLanguage

  /** Whether the editor is read-only or allows editing */
  readonly readOnly?: boolean

  /** Whether to apply border radius to the editor wrapper */
  readonly needBorderRadius?: boolean

  /** Callback fired when the Monaco editor instance is mounted and ready */
  readonly onEditorMount?: (editor: monaco.editor.IStandaloneCodeEditor) => void

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

  /** Whether to show instruction documentation in hover tooltips for TASM */
  readonly showInstructionDocs?: boolean

  /* -------------------------------- Godbolt/Source Mapping -------------------------------- */
  /** Groups of lines to highlight with different colors. Used for source map visualization */
  readonly highlightGroups?: readonly HighlightGroup[]

  /** Individual lines to highlight with hover effect. Used for temporary highlighting */
  readonly hoveredLines?: readonly number[]

  /** Specific text ranges to highlight with precise positioning */
  readonly highlightRanges?: readonly HighlightRange[]

  /** Callback fired when a user hovers over a line. Used for source map highlighting */
  readonly onLineHover?: (line: number | null) => void

  /* -------------------------------- Playground/Editing -------------------------------- */
  /** Callback fired when the code content changes */
  readonly onChange?: (value: string) => void

  /** Error markers to display in the editor. Used for compilation errors in FunC on the Code Explorer page */
  readonly markers?: readonly monaco.editor.IMarkerData[]

  /** Optional gas summation per FunC line to display as inlay hints */
  readonly funcGasByLine?: ReadonlyMap<number, number>

  /** Whether to show the floating tip for the editor */
  readonly needFloatingTip?: boolean

  /** Optional explicit Monaco model path to avoid sharing models between editors */
  readonly modelPath?: string

  /** Use tighter Monaco gutters for embedded read-only trace views. */
  readonly compactGutter?: boolean

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
  onLineClick = () => {},
  onLineHover,
  shouldCenter = true,
  centerLine,
  exitCode,
  readOnly = true,
  onChange,
  language = "tasm",
  highlightGroups = [],
  hoveredLines = [],
  highlightRanges = [],
  markers = [],
  needBorderRadius = true,
  showInstructionDocs = true,
  onEditorMount,
  funcGasByLine,
  needFloatingTip = lineExecutionData && language === "tasm",
  modelPath,
  compactGutter = false,
  sourceDebugVariables,
}) => {
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null)
  const [editorReady, setEditorReady] = useState(false)
  const [isFoldedState, setIsFolded] = useState(false)
  const modelKey =
    modelPath ?? (language === "func" ? "main.fc" : language === "tolk" ? "main.tolk" : "out.tasm")

  const {monaco, isMac} = useMonacoSetup({language})

  const {isCtrlPressed, hoveredLine} = useEditorEvents({
    monaco,
    editorRef,
    lineExecutionData,
    onLineClick,
    onLineHover,
    editorReady,
  })

  const {updateDecorations} = useDecorations({
    monaco,
    highlightLine,
    implicitRetLine,
    lineExecutionData,
    highlightGroups,
    hoveredLines,
    highlightRanges,
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

  useTasmCodeLensProvider({
    monaco,
    editorRef,
    exitCode,
    editorReady,
    enabled: language === "tasm",
  })

  useTasmCompletionProvider({
    monaco,
    editorRef,
    editorReady,
    enabled: language === "tasm",
  })

  useTasmInlayProvider({
    monaco,
    implicitRetLine,
    implicitRetLabel,
    editorRef,
    editorReady,
    enabled: language === "tasm",
  })

  useFuncLanguageProviders({
    monaco,
    editorRef,
    markers,
    enabled: language === "func",
  })

  useImplicitRetInlayProvider({
    monaco,
    editorRef,
    languageId: "func",
    implicitRetLine,
    implicitRetLabel,
    editorReady,
    enabled: language === "func",
  })

  useTolkLanguageProviders({
    monaco,
    editorRef,
    markers,
    enabled: language === "tolk",
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
    if (!editorRef.current || !lineExecutionData || Object.keys(lineExecutionData).length === 0) {
      return
    }
    try {
      editorRef.current.trigger("unfold", "editor.unfoldAll", {})
    } catch {
      /* ignore */
    }
    setIsFolded(true)
  }, [code, language, lineExecutionData])

  useEffect(() => {
    setIsFolded(false)
  }, [lineExecutionData])

  // display gas sum for FunC line of code
  useEffect(() => {
    if (language !== "func" || !editorReady || !editorRef.current || !funcGasByLine) return
    const editor = editorRef.current
    const domNode = editor.getDomNode()
    if (!domNode) return

    const container = document.createElement("div")
    container.style.position = "absolute"
    container.style.pointerEvents = "none"
    container.style.zIndex = "5"
    domNode.appendChild(container)

    const render = () => {
      const layout = editor.getLayoutInfo()
      const scrollTop = editor.getScrollTop()
      const ranges = editor.getVisibleRanges() ?? []
      container.style.left = `${layout.glyphMarginLeft}px`
      container.style.width = `${layout.glyphMarginWidth}px`
      container.style.top = "0px"
      container.innerHTML = ""

      const lines = new Set<number>()
      for (const r of ranges) {
        for (let ln = r.startLineNumber; ln <= r.endLineNumber; ln++) lines.add(ln)
      }

      for (const ln of lines) {
        const gas = funcGasByLine.get(ln)
        if (!gas || gas <= 0) continue
        const top = editor.getTopForLineNumber(ln) - scrollTop
        const el = document.createElement("div")
        el.style.position = "absolute"
        el.style.left = "0"
        el.style.top = `${top}px`
        el.style.height = `${editor.getOption(monaco?.editor.EditorOption.lineHeight ?? 40)}px`
        el.style.display = "flex"
        el.style.alignItems = "center"
        el.style.justifyContent = "end"
        el.style.width = "100%"
        el.style.fontSize = "10px"
        el.style.color = "var(--color-text-secondary)"
        el.style.opacity = "0.9"
        el.style.pointerEvents = "none"
        el.textContent = String(gas)
        container.appendChild(el)
      }
    }

    const disposeScroll = editor.onDidScrollChange(() => render())
    const disposeLayout = editor.onDidLayoutChange(() => render())
    const disposeContent = editor.onDidChangeModelContent(() => render())
    render()

    return () => {
      disposeScroll?.dispose()
      disposeLayout?.dispose()
      disposeContent?.dispose()
      container.remove()
    }
  }, [language, editorReady, editorRef, funcGasByLine, monaco])

  /* ----------------------- folding inactive blocks ----------------------- */
  const handleCollapseInactiveBlocks = useCallback(() => {
    if (isFoldedState) return
    setIsFolded(true)
    collapseInactiveBlocks()
  }, [collapseInactiveBlocks, isFoldedState])

  /* -------------------------------- effects ------------------------------ */
  useEffect(() => {
    if (!editorRef.current) return
    if (isFoldedState) return // don't apply decorations and folds a second time

    updateDecorations(editorRef.current)
    handleCollapseInactiveBlocks()
  }, [lineExecutionData, updateDecorations, handleCollapseInactiveBlocks, isFoldedState])

  useEffect(() => {
    if (!editorReady || !editorRef.current) return

    updateDecorations(editorRef.current)
    handleCollapseInactiveBlocks()
  }, [editorReady, updateDecorations, handleCollapseInactiveBlocks])

  useEffect(() => {
    if (!editorReady || !editorRef.current) return

    const frame = globalThis.requestAnimationFrame(() => {
      if (editorRef.current) {
        editorRef.current.layout()
        updateDecorations(editorRef.current)
      }
    })

    return () => globalThis.cancelAnimationFrame(frame)
  }, [code, editorReady, language, modelKey, updateDecorations])

  // Update decorations on pressed ctrl
  useEffect(() => {
    if (editorRef.current) {
      updateDecorations(editorRef.current)
    }
  }, [isCtrlPressed, updateDecorations])

  // Handle resize events
  useEffect(() => {
    if (!editorReady || !editorRef.current) {
      return
    }

    const handleResize = () => {
      editorRef.current?.layout()
    }

    window.addEventListener("resize", handleResize)
    handleResize()

    return () => {
      window.removeEventListener("resize", handleResize)
    }
  }, [editorReady])

  /* -------------------------------- render ------------------------------- */
  return (
    <>
      <div
        className={
          needBorderRadius
            ? styles.editorWrapperWithBorderRadius
            : styles.editorWrapperWithoutBorderRadius
        }
      >
        <Editor
          className={styles.editor}
          height="100%"
          width="100%"
          language={language}
          path={modelKey}
          value={code}
          saveViewState
          keepCurrentModel
          options={{
            minimap: {enabled: false},
            readOnly,
            lineNumbers: "on",
            automaticLayout: true,
            scrollBeyondLastLine: false,
            wordWrap: "on",
            fontSize: 13.5,
            tabSize: 4,
            insertSpaces: true,
            detectIndentation: false,
            fontFamily: "JetBrains Mono",
            glyphMargin: !compactGutter,
            lineDecorationsWidth: compactGutter ? 6 : undefined,
            lineNumbersMinChars: compactGutter ? 2 : undefined,
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
          onMount={editor => {
            const model = editor.getModel()
            if (monaco && model) {
              model.setEOL(monaco.editor.EndOfLineSequence.LF)
            }

            editorRef.current = editor
            setEditorReady(true)
            if (onEditorMount) {
              onEditorMount(editor)
            }
          }}
          onChange={value => {
            if (onChange !== undefined && value !== undefined) {
              onChange(value)
            }
            if (editorRef.current) {
              updateDecorations(editorRef.current)
            }
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

import {useCallback, useRef} from "react"
import type * as monacoTypes from "monaco-editor"

import type {LinesExecutionData} from "../../../txTrace/hooks"

export interface HighlightGroup {
  readonly lines: number[]
  readonly color?: string
  readonly className?: string
}

export interface HighlightRange {
  readonly line: number
  readonly startColumn: number
  readonly endColumn: number
  readonly color: string
  readonly className?: string
}

interface UseDecorationsOptions {
  readonly monaco: typeof monacoTypes | null
  readonly highlightLine?: number
  readonly implicitRetLine?: number
  readonly lineExecutionData?: LinesExecutionData
  readonly highlightGroups?: readonly HighlightGroup[]
  readonly hoveredLines?: readonly number[]
  readonly highlightRanges?: readonly HighlightRange[]
  readonly isCtrlPressed?: boolean
  readonly hoveredLine?: number | null
  readonly shouldCenter?: boolean
  readonly centerLine?: number
}

interface UseDecorationsReturn {
  readonly updateDecorations: (editor: monacoTypes.editor.IStandaloneCodeEditor) => void
  readonly clearDecorations: (editor: monacoTypes.editor.IStandaloneCodeEditor) => void
}

const createHighlightLineDecoration = (
  monaco: typeof monacoTypes,
  highlightLine: number,
): monacoTypes.editor.IModelDeltaDecoration => ({
  range: new monaco.Range(highlightLine, 1, highlightLine, 1),
  options: {
    isWholeLine: true,
    className: "highlighted-line",
  },
})

const createHighlightGroupDecorations = (
  monaco: typeof monacoTypes,
  highlightGroups: readonly HighlightGroup[],
  totalLines: number,
): monacoTypes.editor.IModelDeltaDecoration[] => {
  const decorations: monacoTypes.editor.IModelDeltaDecoration[] = []

  for (const [index, group] of highlightGroups.entries()) {
    for (const lineNumber of group.lines) {
      if (lineNumber > 0 && lineNumber <= totalLines) {
        const options: monacoTypes.editor.IModelDecorationOptions = {
          isWholeLine: true,
          className: group.className || `source-map-group-${index}`,
        }
        if (group.color) {
          options.overviewRuler = {
            color: group.color,
            position: 1,
          }
        }

        decorations.push({
          range: new monaco.Range(lineNumber, 1, lineNumber, 1),
          options,
        })
      }
    }
  }

  return decorations
}

const createHoveredLinesDecorations = (
  monaco: typeof monacoTypes,
  hoveredLines: readonly number[],
  totalLines: number,
): monacoTypes.editor.IModelDeltaDecoration[] => {
  const decorations: monacoTypes.editor.IModelDeltaDecoration[] = []

  for (const lineNumber of hoveredLines) {
    if (lineNumber > 0 && lineNumber <= totalLines) {
      decorations.push({
        range: new monaco.Range(lineNumber, 1, lineNumber, 1),
        options: {
          isWholeLine: true,
          className: "source-map-hovered-line",
        },
      })
    }
  }

  return decorations
}

const createHighlightRangeDecorations = (
  monaco: typeof monacoTypes,
  highlightRanges: readonly HighlightRange[],
  totalLines: number,
): monacoTypes.editor.IModelDeltaDecoration[] => {
  const decorations: monacoTypes.editor.IModelDeltaDecoration[] = []

  for (const range of highlightRanges) {
    if (range.line > 0 && range.line <= totalLines) {
      decorations.push({
        range: new monaco.Range(range.line, range.startColumn, range.line, range.endColumn),
        options: {
          isWholeLine: false,
          className: range.className ?? "precise-highlight",
          inlineClassName: range.className ?? "precise-highlight",
        },
      })
    }
  }

  return decorations
}

const createExecutionDecorations = (
  monaco: typeof monacoTypes,
  lineExecutionData: LinesExecutionData,
  isCtrlPressed: boolean,
  hoveredLine: number | null,
  model: monacoTypes.editor.ITextModel,
): monacoTypes.editor.IModelDeltaDecoration[] => {
  const decorations: monacoTypes.editor.IModelDeltaDecoration[] = []
  const totalLines = model.getLineCount()

  for (let line = 1; line <= totalLines; line++) {
    const text = model.getLineContent(line)
    const isEmpty = text.trim() === ""
    if (isEmpty) continue

    const wasExecuted = lineExecutionData[line] !== undefined

    if (wasExecuted && isCtrlPressed) {
      const className =
        hoveredLine === line
          ? "clickable-line ctrl-pressed hovered-line"
          : "clickable-line ctrl-pressed"
      decorations.push({
        range: new monaco.Range(line, 1, line, 1),
        options: {
          isWholeLine: true,
          className,
          linesDecorationsClassName: "clickable-line-decoration",
        },
      })
    } else if (!wasExecuted) {
      decorations.push({
        range: new monaco.Range(line, 1, line, model.getLineLength(line) + 1),
        options: {
          isWholeLine: false,
          inlineClassName: "faded-text",
        },
      })
    }
  }

  return decorations
}

export const useDecorations = ({
  monaco,
  highlightLine,
  implicitRetLine,
  lineExecutionData,
  highlightGroups = [],
  hoveredLines = [],
  highlightRanges = [],
  isCtrlPressed = false,
  hoveredLine = null,
  shouldCenter = true,
  centerLine,
}: UseDecorationsOptions): UseDecorationsReturn => {
  const decorationsRef = useRef<string[]>([])

  const updateDecorations = useCallback(
    (editor: monacoTypes.editor.IStandaloneCodeEditor) => {
      if (!monaco || !editor) return

      const model = editor.getModel()
      if (!model) return

      const totalLines = model.getLineCount()
      const allDecorations: monacoTypes.editor.IModelDeltaDecoration[] = []

      try {
        // Highlight the current line
        if (highlightLine !== undefined) {
          allDecorations.push(createHighlightLineDecoration(monaco, highlightLine))
        }

        // Implicit RET marker: render a subtle inline marker on the line below the previous instruction
        if (implicitRetLine !== undefined) {
          const markerLine = Math.min(Math.max(implicitRetLine + 1, 1), totalLines)
          allDecorations.push({
            range: new monaco.Range(markerLine, 1, markerLine, 1),
            options: {
              isWholeLine: true,
              className: "implicit-ret-line",
            },
          })
        }

        // Add source map highlight groups (FunC <-> TASM mappings)
        allDecorations.push(...createHighlightGroupDecorations(monaco, highlightGroups, totalLines))

        // Add hovered lines highlighting
        allDecorations.push(...createHoveredLinesDecorations(monaco, hoveredLines, totalLines))

        // Add highlight ranges
        allDecorations.push(...createHighlightRangeDecorations(monaco, highlightRanges, totalLines))

        // Add execution-based decorations
        if (lineExecutionData && Object.keys(lineExecutionData).length > 0) {
          allDecorations.push(
            ...createExecutionDecorations(
              monaco,
              lineExecutionData,
              isCtrlPressed,
              hoveredLine,
              model,
            ),
          )
        }

        // noinspection JSDeprecatedSymbols
        decorationsRef.current = editor.deltaDecorations(decorationsRef.current, allDecorations)

        // Center on highlighted line; if absent and implicit RET is present, center on it
        if (shouldCenter) {
          if (centerLine !== undefined) {
            editor.revealLineInCenterIfOutsideViewport(centerLine)
          } else if (highlightLine !== undefined) {
            editor.revealLineInCenterIfOutsideViewport(highlightLine)
          } else if (implicitRetLine !== undefined) {
            const markerLine = Math.min(Math.max(implicitRetLine + 1, 1), totalLines)
            editor.revealLineInCenterIfOutsideViewport(markerLine)
          }
        }
      } catch (error) {
        console.error("Failed to update decorations:", error)
      }
    },
    [
      monaco,
      highlightLine,
      highlightGroups,
      hoveredLines,
      highlightRanges,
      lineExecutionData,
      shouldCenter,
      centerLine,
      isCtrlPressed,
      hoveredLine,
      implicitRetLine,
    ],
  )

  const clearDecorations = useCallback((editor: monacoTypes.editor.IStandaloneCodeEditor) => {
    if (!editor) return

    try {
      decorationsRef.current = editor.deltaDecorations(decorationsRef.current, [])
    } catch (error) {
      console.error("Failed to clear decorations:", error)
    }
  }, [])

  return {
    updateDecorations,
    clearDecorations,
  }
}

import {useCallback, type RefObject} from "react"
import type * as monacoTypes from "monaco-editor"

import type {LinesExecutionData} from "../../../txTrace/hooks"

interface UseFoldingOptions {
  readonly monaco: typeof monacoTypes | null
  readonly editorRef: RefObject<monacoTypes.editor.IStandaloneCodeEditor | null>
  readonly lineExecutionData?: LinesExecutionData
}

interface UseFoldingReturn {
  readonly collapseInactiveBlocks: () => void
}

interface CodeBlock {
  readonly start: number
  readonly end: number
}

interface FoldingRange {
  readonly start: number
  readonly end: number
}

export const useFolding = ({
  monaco,
  editorRef,
  lineExecutionData,
}: UseFoldingOptions): UseFoldingReturn => {
  const collapseInactiveBlocks = useCallback(() => {
    if (!editorRef.current || !monaco) return

    try {
      editorRef.current.trigger("unfold", "editor.unfoldAll", {})
    } catch {
      // ignored
    }

    const model = editorRef.current.getModel()
    if (!model) return

    if (!lineExecutionData || Object.keys(lineExecutionData).length === 0) return

    const totalLines = model.getLineCount()
    const foldingRanges: FoldingRange[] = []
    const codeBlocks: CodeBlock[] = []
    const blockStack: number[] = []

    for (let line = 1; line <= totalLines; line++) {
      const lineText = model.getLineContent(line)
      const openBraces = (lineText.match(/\{/g) ?? []).length
      const closeBraces = (lineText.match(/}/g) ?? []).length
      for (let i = 0; i < openBraces; i++) {
        blockStack.push(line)
      }
      for (let i = 0; i < closeBraces; i++) {
        if (blockStack.length > 0) {
          const startLine = blockStack.pop() ?? 0
          codeBlocks.push({start: startLine, end: line})
        }
      }
    }

    for (const block of codeBlocks) {
      let allLinesInactive = true
      for (let line = block.start; line <= block.end; line++) {
        const lineText = model.getLineContent(line)
        const isActiveLine = lineExecutionData[line] !== undefined
        const isEmpty = lineText.trim() === ""
        if (isActiveLine && !isEmpty) {
          allLinesInactive = false
          break
        }
      }
      if (allLinesInactive && block.start < block.end) {
        foldingRanges.push({start: block.start, end: block.end})
      }
    }

    setTimeout(() => {
      for (const range of foldingRanges) {
        try {
          editorRef.current?.trigger("fold", "editor.fold", {
            levels: 1,
            selectionLines: [range.start],
          })
        } catch {
          // ignored
        }
      }
    }, 100)
  }, [editorRef, monaco, lineExecutionData])

  return {
    collapseInactiveBlocks,
  }
}

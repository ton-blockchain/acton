import {useCallback, useEffect, useState, type RefObject} from "react"
import type * as monacoTypes from "monaco-editor"
import {editor} from "monaco-editor"

import type {LinesExecutionData} from "../../../txTrace/hooks"

interface UseEditorEventsOptions {
  readonly monaco: typeof monacoTypes | null
  readonly editorRef: RefObject<monacoTypes.editor.IStandaloneCodeEditor | null>
  readonly lineExecutionData?: LinesExecutionData
  readonly onLineClick?: (line: number) => void
  readonly onLineHover?: (line: number | null) => void
  readonly editorReady?: boolean
}

interface UseEditorEventsReturn {
  readonly isCtrlPressed: boolean
  readonly hoveredLine: number | null
}

export const useEditorEvents = ({
  monaco,
  editorRef,
  lineExecutionData,
  onLineClick,
  onLineHover,
  editorReady = true,
}: UseEditorEventsOptions): UseEditorEventsReturn => {
  const [isCtrlPressed, setIsCtrlPressed] = useState(false)
  const [hoveredLine, setHoveredLine] = useState<number | null>(null)

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !isCtrlPressed) {
        setIsCtrlPressed(true)
      }

      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault()
        e.stopPropagation()
      }
    },
    [isCtrlPressed],
  )

  const handleKeyUp = useCallback(
    (e: KeyboardEvent) => {
      if (!e.ctrlKey && !e.metaKey && isCtrlPressed) {
        setIsCtrlPressed(false)
        setHoveredLine(null)
      }
    },
    [isCtrlPressed],
  )

  const handleBlur = useCallback(() => {
    if (isCtrlPressed) {
      setIsCtrlPressed(false)
      setHoveredLine(null)
    }
  }, [isCtrlPressed])

  // Line click handler
  useEffect(() => {
    if (!editorRef.current || !monaco || !editorReady) return

    const disposable = editorRef.current.onMouseDown((e: editor.IEditorMouseEvent) => {
      if (
        e.target.type !== monaco.editor.MouseTargetType.GUTTER_LINE_NUMBERS &&
        e.event.leftButton &&
        (e.event.ctrlKey || e.event.metaKey)
      ) {
        const lineNumber = e.target.position?.lineNumber

        if (
          lineNumber &&
          lineExecutionData &&
          lineExecutionData[lineNumber] !== undefined &&
          onLineClick
        ) {
          onLineClick(lineNumber)
        }
      }
    })

    return () => disposable.dispose()
  }, [editorRef, lineExecutionData, onLineClick, monaco, editorReady])

  // Combined mouse move handler for both Ctrl+click hover and source map hover
  useEffect(() => {
    if (!monaco || !editorRef.current || !editorReady) return

    const disposable = editorRef.current.onMouseMove((e: editor.IEditorMouseEvent) => {
      const lineNumber = e.target.position?.lineNumber

      // Handle Ctrl+click hover (for clickable lines)
      if (
        isCtrlPressed &&
        lineNumber &&
        lineExecutionData &&
        lineExecutionData[lineNumber] !== undefined
      ) {
        setHoveredLine(lineNumber)
      } else if (isCtrlPressed) {
        setHoveredLine(null)
      }

      // Handle source map hover (always active if onLineHover is provided)
      if (onLineHover && lineNumber) {
        onLineHover(lineNumber)
      }
    })

    const handleMouseLeave = () => {
      if (isCtrlPressed) {
        setHoveredLine(null)
      }
      if (onLineHover) {
        onLineHover(null)
      }
    }

    const editorDom = editorRef.current.getDomNode()
    if (editorDom) {
      editorDom.addEventListener("mouseleave", handleMouseLeave)
    }

    return () => {
      disposable.dispose()
      editorDom?.removeEventListener("mouseleave", handleMouseLeave)
    }
  }, [monaco, editorRef, isCtrlPressed, lineExecutionData, onLineHover, editorReady])

  useEffect(() => {
    document.addEventListener("keydown", handleKeyDown)
    document.addEventListener("keyup", handleKeyUp)
    window.addEventListener("blur", handleBlur)

    return () => {
      document.removeEventListener("keydown", handleKeyDown)
      document.removeEventListener("keyup", handleKeyUp)
      window.removeEventListener("blur", handleBlur)
    }
  }, [handleKeyDown, handleKeyUp, handleBlur])

  return {
    isCtrlPressed,
    hoveredLine,
  }
}

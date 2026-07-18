import {useCallback, useEffect, useState, type RefObject} from "react"
import type * as monacoTypes from "monaco-editor"
import type {editor} from "monaco-editor"

import type {LinesExecutionData} from "../../../txTrace/hooks"

interface UseEditorEventsOptions {
  readonly monaco: typeof monacoTypes | null
  readonly editorRef: RefObject<monacoTypes.editor.IStandaloneCodeEditor | null>
  readonly lineExecutionData?: LinesExecutionData
  readonly onLineClick?: (line: number) => void
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
  editorReady = true,
}: UseEditorEventsOptions): UseEditorEventsReturn => {
  const [isCtrlPressed, setIsCtrlPressed] = useState(false)
  const [hoveredLine, setHoveredLine] = useState<number | null>(null)

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !isCtrlPressed) {
        setIsCtrlPressed(true)
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

  useEffect(() => {
    if (!editorRef.current || !monaco || !editorReady || !lineExecutionData || !onLineClick) {
      return
    }

    const disposable = editorRef.current.onMouseDown((e: editor.IEditorMouseEvent) => {
      if (
        e.target.type !== monaco.editor.MouseTargetType.GUTTER_LINE_NUMBERS &&
        e.event.leftButton &&
        (e.event.ctrlKey || e.event.metaKey)
      ) {
        const lineNumber = e.target.position?.lineNumber

        if (lineNumber && lineExecutionData[lineNumber] !== undefined) {
          onLineClick(lineNumber)
        }
      }
    })

    return () => disposable.dispose()
  }, [editorRef, lineExecutionData, onLineClick, monaco, editorReady])

  useEffect(() => {
    if (!editorRef.current || !editorReady || !lineExecutionData || !onLineClick) {
      return
    }

    const disposable = editorRef.current.onMouseMove((e: editor.IEditorMouseEvent) => {
      const lineNumber = e.target.position?.lineNumber

      if (isCtrlPressed && lineNumber && lineExecutionData[lineNumber] !== undefined) {
        setHoveredLine(lineNumber)
      } else if (isCtrlPressed) {
        setHoveredLine(null)
      }
    })

    const handleMouseLeave = () => {
      if (isCtrlPressed) {
        setHoveredLine(null)
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
  }, [editorRef, isCtrlPressed, lineExecutionData, onLineClick, editorReady])

  useEffect(() => {
    if (!lineExecutionData || !onLineClick) {
      return
    }

    document.addEventListener("keydown", handleKeyDown)
    document.addEventListener("keyup", handleKeyUp)
    window.addEventListener("blur", handleBlur)

    return () => {
      document.removeEventListener("keydown", handleKeyDown)
      document.removeEventListener("keyup", handleKeyUp)
      window.removeEventListener("blur", handleBlur)
    }
  }, [handleKeyDown, handleKeyUp, handleBlur, lineExecutionData, onLineClick])

  return {
    isCtrlPressed,
    hoveredLine,
  }
}

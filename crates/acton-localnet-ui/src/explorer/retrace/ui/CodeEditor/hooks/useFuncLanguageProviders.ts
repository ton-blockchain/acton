import {useEffect, type RefObject} from "react"
import type * as monacoTypes from "monaco-editor"

interface UseFuncLanguageProvidersOptions {
  readonly monaco: typeof monacoTypes | null
  readonly editorRef: RefObject<monacoTypes.editor.IStandaloneCodeEditor | null>
  readonly markers?: readonly monacoTypes.editor.IMarkerData[]
  readonly enabled?: boolean
}

export const useFuncLanguageProviders = ({
  monaco,
  editorRef,
  markers = [],
  enabled,
}: UseFuncLanguageProvidersOptions): void => {
  useEffect(() => {
    if (!monaco || !editorRef.current || !enabled) return

    const model = editorRef.current.getModel()
    if (!model) return

    monaco.editor.setModelMarkers(model, "FunC", [...markers])

    return () => {
      monaco.editor.setModelMarkers(model, "FunC", [])
    }
  }, [monaco, markers, editorRef, enabled])
}

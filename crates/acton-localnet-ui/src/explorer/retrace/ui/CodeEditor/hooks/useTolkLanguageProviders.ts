import {useEffect, type RefObject} from "react"
import type * as monacoTypes from "monaco-editor"

interface UseTolkLanguageProvidersOptions {
  readonly monaco: typeof monacoTypes | null
  readonly editorRef: RefObject<monacoTypes.editor.IStandaloneCodeEditor | null>
  readonly markers?: readonly monacoTypes.editor.IMarkerData[]
  readonly enabled?: boolean
}

export const useTolkLanguageProviders = ({
  monaco,
  editorRef,
  markers = [],
  enabled,
}: UseTolkLanguageProvidersOptions): void => {
  useEffect(() => {
    if (!monaco || !editorRef.current || !enabled) return

    const model = editorRef.current.getModel()
    if (!model) return

    monaco.editor.setModelMarkers(model, "Tolk", [...markers])

    return () => {
      monaco.editor.setModelMarkers(model, "Tolk", [])
    }
  }, [monaco, markers, editorRef, enabled])
}

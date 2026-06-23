import {useEffect, type RefObject} from "react"
import type * as monacoTypes from "monaco-editor"

interface UseImplicitRetInlayProviderOptions {
  readonly monaco: typeof monacoTypes | null
  readonly editorRef: RefObject<monacoTypes.editor.IStandaloneCodeEditor | null>
  readonly languageId: string
  readonly implicitRetLine?: number
  readonly implicitRetLabel?: string
  readonly editorReady: boolean
  readonly enabled?: boolean
}

export const useImplicitRetInlayProvider = ({
  monaco,
  editorRef,
  languageId,
  implicitRetLine,
  implicitRetLabel,
  editorReady,
  enabled,
}: UseImplicitRetInlayProviderOptions): void => {
  useEffect(() => {
    if (!monaco || !editorRef.current || !editorReady || !enabled) return

    const provider = monaco.languages.registerInlayHintsProvider(languageId, {
      displayName: "TxTracer Implicit RET",
      provideInlayHints(model) {
        if (implicitRetLine === undefined) {
          return undefined
        }

        const line = Math.min(Math.max(implicitRetLine + 1, 1), model.getLineCount())
        const ranges = editorRef.current?.getVisibleRanges() ?? []

        if (ranges.some(range => range.containsPosition({lineNumber: line, column: 0}))) {
          const hint: monacoTypes.languages.InlayHint = {
            label: implicitRetLabel ?? "↵ implicit RET",
            position: {lineNumber: line, column: model.getLineLength(line) + 1},
            kind: monaco.languages.InlayHintKind.Type,
            tooltip: "Implicit return from a continuation",
            paddingLeft: true,
            paddingRight: true,
          }
          return {
            hints: [hint],
            dispose: () => {},
          }
        }

        return undefined
      },
    })

    return () => {
      provider.dispose()
    }
  }, [monaco, editorRef, languageId, implicitRetLine, implicitRetLabel, editorReady, enabled])
}

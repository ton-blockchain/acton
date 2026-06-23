import {type RefObject, useEffect} from "react"
import type * as monacoTypes from "monaco-editor"

import {languages} from "monaco-editor"

import {TASM_LANGUAGE_ID} from "../languages"

interface UseTasmInlayProviderOptions {
  readonly monaco: typeof monacoTypes | null
  readonly implicitRetLine?: number
  readonly implicitRetLabel?: string
  readonly editorRef: RefObject<monacoTypes.editor.IStandaloneCodeEditor | null>
  readonly editorReady: boolean
  readonly enabled?: boolean
}

export const useTasmInlayProvider = ({
  monaco,
  implicitRetLine,
  implicitRetLabel,
  editorRef,
  editorReady,
  enabled,
}: UseTasmInlayProviderOptions): void => {
  useEffect(() => {
    if (!monaco || !editorRef.current || !editorReady || !enabled) return

    const provider = monaco.languages.registerInlayHintsProvider(TASM_LANGUAGE_ID, {
      displayName: "TxTracer Inlays",
      provideInlayHints(model) {
        if (implicitRetLine === undefined) {
          return undefined
        }

        const line = Math.min(Math.max(implicitRetLine + 1, 1), model.getLineCount())
        const ranges = editorRef.current?.getVisibleRanges() ?? []

        if (ranges.some(range => range.containsPosition({lineNumber: line, column: 0}))) {
          const hint: languages.InlayHint = {
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
  }, [monaco, implicitRetLine, implicitRetLabel, editorReady, enabled, editorRef])
}

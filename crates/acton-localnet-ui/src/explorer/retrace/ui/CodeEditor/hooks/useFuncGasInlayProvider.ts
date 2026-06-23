import {useEffect, type RefObject} from "react"
import type * as monacoTypes from "monaco-editor"

interface UseFuncGasInlayProviderOptions {
  readonly monaco: typeof monacoTypes | null
  readonly editorRef: RefObject<monacoTypes.editor.IStandaloneCodeEditor | null>
  readonly editorReady: boolean
  readonly gasByLine?: ReadonlyMap<number, number>
  readonly enabled?: boolean
}

export function useFuncGasInlayProvider({
  monaco,
  editorRef,
  editorReady,
  gasByLine,
  enabled,
}: UseFuncGasInlayProviderOptions): void {
  useEffect(() => {
    if (!monaco || !editorRef.current || !editorReady || !enabled || !gasByLine) return

    const provider = monaco.languages.registerInlayHintsProvider("func", {
      displayName: "TxTracer FunC Gas",
      provideInlayHints(model) {
        const ranges = editorRef.current?.getVisibleRanges() ?? []
        if (ranges.length === 0) return {hints: [], dispose: () => {}}

        const candidateLines = new Set<number>()
        for (const r of ranges) {
          for (let ln = r.startLineNumber; ln <= r.endLineNumber; ln++) {
            candidateLines.add(ln)
          }
        }

        const hints: monacoTypes.languages.InlayHint[] = []
        for (const ln of candidateLines) {
          const gas = gasByLine.get(ln)
          if (!gas || gas <= 0) continue
          hints.push({
            label: `gas: ${gas}`,
            position: {lineNumber: ln, column: model.getLineLength(ln) + 1},
            kind: monaco.languages.InlayHintKind.Type,
            paddingLeft: true,
            paddingRight: true,
          })
        }

        return {hints, dispose: () => {}}
      },
    })

    return () => {
      provider.dispose()
    }
  }, [monaco, editorRef, editorReady, gasByLine, enabled])
}

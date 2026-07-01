import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import type * as monacoTypes from "monaco-editor"
import {type RefObject, useEffect} from "react"

import {formatExitCode} from "../../../txTrace/lib/exitCodeFormatting"
import type {ExitCode} from "../../../txTrace/lib/types"

export interface CodeLensAnnotation {
  readonly line: number
  readonly title: string
  readonly tooltip?: string
  readonly id?: string
}

interface UseCodeLensProviderOptions {
  readonly monaco: typeof monacoTypes | null
  readonly editorRef: RefObject<monacoTypes.editor.IStandaloneCodeEditor | null>
  readonly languageId: string
  readonly exitCode?: ExitCode
  readonly compilerAbi?: ContractABI
  readonly annotation?: CodeLensAnnotation
  readonly editorReady: boolean
  readonly enabled?: boolean
}

function exitCodeAnnotation(
  exitCode: ExitCode | undefined,
  compilerAbi: ContractABI | undefined,
): CodeLensAnnotation | undefined {
  if (!exitCode?.info?.loc?.line && exitCode?.info?.loc?.line !== 0) {
    return undefined
  }

  const line = exitCode.info.loc.line + 1
  const formatted = formatExitCode(exitCode.num, {
    compilerAbi,
    vmDescription: exitCode.description,
  })

  return {
    line,
    id: `exitCode-${line}`,
    title: formatted.title,
    tooltip: formatted.tooltip,
  }
}

export const useCodeLensProvider = ({
  monaco,
  editorRef,
  languageId,
  exitCode,
  compilerAbi,
  annotation,
  editorReady,
  enabled,
}: UseCodeLensProviderOptions): void => {
  useEffect(() => {
    if (!monaco || !editorRef.current || !editorReady || !enabled) return

    const provider = monaco.languages.registerCodeLensProvider(languageId, {
      provideCodeLenses: model => {
        if (editorRef.current?.getModel() !== model) {
          return {
            lenses: [],
            dispose: () => {},
          }
        }

        const codeLens = annotation ?? exitCodeAnnotation(exitCode, compilerAbi)
        if (!codeLens) {
          return {
            lenses: [],
            dispose: () => {},
          }
        }

        const line = codeLens.line
        if (line <= 0 || line > model.getLineCount()) {
          return {
            lenses: [],
            dispose: () => {},
          }
        }

        const lenses: monacoTypes.languages.CodeLens[] = [
          {
            range: new monaco.Range(line, 1, line, 1),
            id: codeLens.id ?? `codeLens-${line}`,
            command: {
              id: "noop",
              title: codeLens.title,
              tooltip: codeLens.tooltip ?? codeLens.title,
            },
          },
        ]

        return {
          lenses: lenses,
          dispose: () => {},
        }
      },
      resolveCodeLens: (_model, codeLens) => {
        return codeLens
      },
    })

    return () => {
      provider.dispose()
    }
  }, [monaco, editorRef, languageId, exitCode, compilerAbi, annotation, editorReady, enabled])
}

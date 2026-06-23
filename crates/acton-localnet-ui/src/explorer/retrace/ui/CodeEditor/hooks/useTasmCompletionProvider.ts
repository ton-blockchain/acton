import {useEffect, type RefObject} from "react"
import type * as monacoTypes from "monaco-editor"
import {editor, languages, Position} from "monaco-editor"

import {instructionSpecification} from "@retrace/tasm/lib"

import {TASM_LANGUAGE_ID} from "../languages"

interface UseTasmCompletionProviderOptions {
  readonly monaco: typeof monacoTypes | null
  readonly editorRef: RefObject<monacoTypes.editor.IStandaloneCodeEditor | null>
  readonly enabled?: boolean
  readonly editorReady: boolean
}

export const useTasmCompletionProvider = ({
  monaco,
  editorRef,
  editorReady,
  enabled = true,
}: UseTasmCompletionProviderOptions): void => {
  useEffect(() => {
    if (!monaco || !editorRef.current || !editorReady || !enabled) return

    const provider = monaco.languages.registerCompletionItemProvider(TASM_LANGUAGE_ID, {
      triggerCharacters: [],
      provideCompletionItems(model: editor.ITextModel, position: Position) {
        const data = instructionSpecification()

        if (!data) {
          return {suggestions: []}
        }

        const word = model.getWordUntilPosition(position)
        const range = {
          startLineNumber: position.lineNumber,
          endLineNumber: position.lineNumber,
          startColumn: word.startColumn,
          endColumn: word.endColumn,
        }

        const inputText = word.word.toUpperCase()
        const suggestions: languages.CompletionItem[] = []

        for (const instruction of data.instructions) {
          const name = instruction.name

          if (inputText && !name.startsWith(inputText)) {
            continue
          }

          const operands = instruction.description.operands

          suggestions.push({
            label: name,
            kind: monaco.languages.CompletionItemKind.Function,
            insertText: name + (operands.length > 0 ? " " : ""),
            range,
            sortText: `0_${name}`,
            filterText: name,
          })
        }

        return {suggestions}
      },
    })

    return () => {
      provider.dispose()
    }
  }, [monaco, editorRef, editorReady, enabled])
}

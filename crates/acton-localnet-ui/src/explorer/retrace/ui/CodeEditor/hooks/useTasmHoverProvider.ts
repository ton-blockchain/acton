import {useEffect} from "react"
import type * as monacoTypes from "monaco-editor"
import {editor, type IMarkdownString, Position} from "monaco-editor"

import {findInstruction, generateAsmDoc} from "../../../tasm/lib"

import type {LinesExecutionData} from "../../../txTrace/hooks"

import {TASM_LANGUAGE_ID} from "../languages"

const CONTROL_REGISTERS: Readonly<
  Record<string, {readonly type: string; readonly description: string}>
> = {
  c0: {
    type: "Continuation",
    description: "Stores the next or return continuation, similar to a return address.",
  },
  c1: {
    type: "Continuation",
    description: "Stores the alternative continuation.",
  },
  c2: {
    type: "Continuation",
    description: "Contains the exception handler (continuation).",
  },
  c3: {
    type: "Continuation",
    description: "Holds the current dictionary (hashmap of function codes) as a continuation.",
  },
  c4: {
    type: "Cell",
    description: "Stores persistent data (contract's data section).",
  },
  c5: {
    type: "Cell",
    description: "Contains output actions.",
  },
  c7: {
    type: "Tuple",
    description: "Stores temporary data.",
  },
} as const

interface UseTasmHoverProviderOptions {
  readonly monaco: typeof monacoTypes | null
  readonly lineExecutionData?: LinesExecutionData
  readonly showInstructionDocs?: boolean
  readonly editorReady?: boolean
  readonly enabled?: boolean
}

export const useTasmHoverProvider = ({
  monaco,
  lineExecutionData,
  showInstructionDocs = true,
  editorReady,
  enabled,
}: UseTasmHoverProviderOptions): void => {
  useEffect(() => {
    if (!monaco || !editorReady || !enabled) return

    const provider = monaco.languages.registerHoverProvider(TASM_LANGUAGE_ID, {
      provideHover(model: editor.ITextModel, position: Position) {
        const word = model.getWordAtPosition(position)
        const lineNumber = position.lineNumber
        const hoverContents: IMarkdownString[] = []

        if (word) {
          const crInfo = CONTROL_REGISTERS[word.word]
          if (crInfo) {
            hoverContents.push({
              value: `**Control register ${word.word} (${crInfo.type})**\n\n${crInfo.description}`,
            })
          }

          const lineContent = model.getLineContent(lineNumber)
          const tokens = monaco.editor.tokenize(lineContent, TASM_LANGUAGE_ID)[0]
          let tokenType = ""
          for (let i = 0; i < tokens.length; i++) {
            const token = tokens[i]
            const start = token.offset + 1
            const end = i + 1 < tokens.length ? tokens[i + 1].offset + 1 : lineContent.length + 1
            if (position.column >= start && position.column < end) {
              tokenType = token.type
              break
            }
          }

          if (tokenType.includes("instruction") && showInstructionDocs) {
            const instructionInfo = findInstruction(word.word)
            if (instructionInfo) {
              const asmDoc = generateAsmDoc(instructionInfo)
              if (asmDoc) {
                hoverContents.push({value: asmDoc})
              }
            }

            if (lineExecutionData) {
              const executionData = lineExecutionData[lineNumber]

              if (hoverContents.length > 0) {
                hoverContents.push({value: "---"})
              }

              if (executionData === undefined) {
                hoverContents.push({value: "**Not executed**"})
              } else {
                hoverContents.push({value: `**Executions:** ${executionData.executions}`})
              }
            }
          }
        }

        if (hoverContents.length > 0) {
          return {
            range: word
              ? new monaco.Range(
                  position.lineNumber,
                  word.startColumn,
                  position.lineNumber,
                  word.endColumn,
                )
              : new monaco.Range(
                  position.lineNumber,
                  1,
                  position.lineNumber,
                  model.getLineLength(position.lineNumber) + 1,
                ),
            contents: hoverContents,
          }
        }
        return null
      },
    })

    return () => {
      provider.dispose()
    }
  }, [monaco, lineExecutionData, showInstructionDocs, enabled, editorReady])
}

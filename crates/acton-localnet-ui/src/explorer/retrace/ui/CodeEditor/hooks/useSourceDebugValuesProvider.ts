import type * as monacoTypes from "monaco-editor"
import {editor, type IMarkdownString, Position} from "monaco-editor"
import {type MutableRefObject, useEffect, useMemo} from "react"

export interface SourceDebugVariableValue {
  readonly name: string
  readonly value: string
  readonly type: string | null
  readonly children: readonly SourceDebugVariableValue[]
}

interface SourceDebugValueRecord {
  readonly expression: string
  readonly name: string
  readonly value: string
  readonly type: string | null
  readonly children: readonly SourceDebugVariableValue[]
}

interface UseSourceDebugValuesProviderOptions {
  readonly monaco: typeof monacoTypes | null
  readonly editorRef: MutableRefObject<monacoTypes.editor.IStandaloneCodeEditor | null>
  readonly editorReady: boolean
  readonly languageId: string
  readonly variables?: readonly SourceDebugVariableValue[]
  readonly enabled?: boolean
}

const MAX_HOVER_CHILDREN = 16
const MAX_HOVER_DEPTH = 4
const MAX_HOVER_SCALAR_LENGTH = 240
const IDENTIFIER_PATTERN = /[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*)*/g
const EMPTY_SOURCE_DEBUG_VARIABLES: readonly SourceDebugVariableValue[] = []

function truncateText(value: string, maxLength: number): string {
  if (value.length <= maxLength) {
    return value
  }

  return `${value.slice(0, Math.max(0, maxLength - 3))}...`
}

function scalarPreview(value: string, maxLength: number): string {
  return truncateText(value.replace(/\s+/g, " ").trim(), maxLength)
}

function isPlaceholderValue(value: string): boolean {
  const normalized = value.trim()
  return normalized === "" || normalized === "{...}" || normalized === "..."
}

function buildSourceDebugValueRecords(
  variables: readonly SourceDebugVariableValue[],
  parentExpression = "",
): SourceDebugValueRecord[] {
  const records: SourceDebugValueRecord[] = []

  for (const variable of variables) {
    const expression = parentExpression ? `${parentExpression}.${variable.name}` : variable.name
    records.push({
      expression,
      name: variable.name,
      value: variable.value,
      type: variable.type,
      children: variable.children,
    })
    records.push(...buildSourceDebugValueRecords(variable.children, expression))
  }

  return records
}

function detailedVariableValue(variable: SourceDebugVariableValue, depth = 0): string {
  if (variable.children.length === 0 || depth >= MAX_HOVER_DEPTH) {
    return scalarPreview(variable.value, MAX_HOVER_SCALAR_LENGTH)
  }

  const childIndent = "  ".repeat(depth)
  const childLines = variable.children.slice(0, MAX_HOVER_CHILDREN).map(child => {
    const typeLabel = child.type ? `{${child.type}} ` : ""
    return `${childIndent}${child.name}: ${typeLabel}${detailedVariableValue(child, depth + 1)}`
  })

  if (variable.children.length > MAX_HOVER_CHILDREN) {
    childLines.push(`${childIndent}...`)
  }

  const valuePrefix = isPlaceholderValue(variable.value)
    ? ""
    : scalarPreview(variable.value, MAX_HOVER_SCALAR_LENGTH)
  return valuePrefix ? `${valuePrefix}\n${childLines.join("\n")}` : childLines.join("\n")
}

function expressionMap(
  records: readonly SourceDebugValueRecord[],
): ReadonlyMap<string, SourceDebugValueRecord> {
  return new Map(records.map(record => [record.expression, record]))
}

function hoverRecordAtPosition(
  lineContent: string,
  position: Position,
  recordsByExpression: ReadonlyMap<string, SourceDebugValueRecord>,
):
  | {
      readonly record: SourceDebugValueRecord
      readonly startColumn: number
      readonly endColumn: number
    }
  | undefined {
  for (const match of lineContent.matchAll(IDENTIFIER_PATTERN)) {
    const token = match[0]
    const startColumn = (match.index ?? 0) + 1
    const endColumn = startColumn + token.length
    if (position.column < startColumn || position.column > endColumn) {
      continue
    }

    const fullRecord = recordsByExpression.get(token)
    if (fullRecord) {
      return {
        record: fullRecord,
        startColumn,
        endColumn,
      }
    }

    const tokenOffset = Math.max(0, Math.min(token.length - 1, position.column - startColumn))
    const nextDotOffset = token.indexOf(".", tokenOffset)
    const expressionEndOffset = nextDotOffset === -1 ? token.length : nextDotOffset
    const expression = token.slice(0, expressionEndOffset)
    const record = recordsByExpression.get(expression)
    if (record) {
      return {
        record,
        startColumn,
        endColumn: startColumn + expression.length,
      }
    }
  }

  return undefined
}

function hoverContents(record: SourceDebugValueRecord): IMarkdownString[] {
  const contents: IMarkdownString[] = [{value: `**${record.expression}**`}]

  if (record.type) {
    contents.push({value: `\`${record.type}\``})
  }

  contents.push({value: `\`\`\`text\n${detailedVariableValue(record)}\n\`\`\``})
  return contents
}

export const useSourceDebugValuesProvider = ({
  monaco,
  editorRef,
  editorReady,
  languageId,
  variables = EMPTY_SOURCE_DEBUG_VARIABLES,
  enabled = true,
}: UseSourceDebugValuesProviderOptions): void => {
  const records = useMemo(() => buildSourceDebugValueRecords(variables), [variables])
  const recordsByExpression = useMemo(() => expressionMap(records), [records])

  useEffect(() => {
    if (!monaco || !editorReady || !enabled || records.length === 0) {
      return
    }

    const editorInstance = editorRef.current
    const currentModel = editorInstance?.getModel()
    if (!currentModel) {
      return
    }

    const provider = monaco.languages.registerHoverProvider(languageId, {
      provideHover(model: editor.ITextModel, position: Position) {
        if (model.uri.toString() !== currentModel.uri.toString()) {
          return null
        }

        const match = hoverRecordAtPosition(
          model.getLineContent(position.lineNumber),
          position,
          recordsByExpression,
        )
        if (!match) {
          return null
        }

        return {
          range: new monaco.Range(
            position.lineNumber,
            match.startColumn,
            position.lineNumber,
            match.endColumn,
          ),
          contents: hoverContents(match.record),
        }
      },
    })

    return () => provider.dispose()
  }, [monaco, editorRef, editorReady, enabled, languageId, records.length, recordsByExpression])
}

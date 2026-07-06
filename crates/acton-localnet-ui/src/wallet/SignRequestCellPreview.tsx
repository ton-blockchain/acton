import {KeyRound} from "lucide-react"
import {parseTLB, type ParsedCell} from "@ton-community/tlb-runtime"
import {Cell} from "@ton/core"
import type {CSSProperties, FC} from "react"
import {useMemo} from "react"
import {formatUnits, type SignDataRequestEvent} from "@ton/walletkit"

import styles from "../dashboard/pages/WalletsPage.module.css"

type SignRequestCellData = Extract<SignDataRequestEvent["preview"]["data"], {type: "cell"}>

interface SignRequestCellPreviewProps {
  readonly preview: SignRequestCellData
}

interface ParsedCellField {
  readonly label: string
  readonly value: string
  readonly depth: number
  readonly tone?: "muted" | "error"
}

interface ParsedCellPreview {
  readonly title: string
  readonly hash: string
  readonly bits: number
  readonly refs: number
  readonly fields: ParsedCellField[]
  readonly error?: string
}

export const SignRequestCellPreview: FC<SignRequestCellPreviewProps> = ({preview}) => {
  const parsedCell = useMemo(
    () => parseCellPreview(preview.value.content, preview.value.schema),
    [preview],
  )

  return (
    <>
      <div className={styles.messageItem}>
        <KeyRound size={16} />
        <div>
          <div className={styles.messageAddress}>CELL</div>
          <div className={styles.permissionDescription}>
            {parsedCell
              ? `${parsedCell.title} · ${parsedCell.bits} bits · ${parsedCell.refs} refs`
              : preview.value.schema || "TON Cell payload"}
          </div>
        </div>
      </div>

      {parsedCell && (
        <details className={styles.signPreviewPanel} open>
          <summary className={styles.signPreviewHeader}>
            <span>Parsed Cell</span>
            <span className={styles.signPreviewHash}>{shortenHash(parsedCell.hash)}</span>
          </summary>
          {parsedCell.error && (
            <div className={`${styles.signPreviewNotice} ${styles.signPreviewNoticeError}`}>
              {parsedCell.error}
            </div>
          )}
          <div className={styles.signPreviewRows}>
            {parsedCell.fields.map((field, index) => (
              <div
                key={`${field.label}-${index}`}
                className={`${styles.signPreviewRow} ${
                  field.tone === "error" ? styles.signPreviewRowError : ""
                }`}
                style={{"--sign-preview-depth": field.depth} as CSSProperties}
              >
                <span className={styles.signPreviewLabel}>{field.label}</span>
                <span className={styles.signPreviewValue}>{field.value}</span>
              </div>
            ))}
          </div>
        </details>
      )}

      <details className={styles.signPreviewDetails}>
        <summary>Schema</summary>
        <pre>{preview.value.schema || "No schema"}</pre>
      </details>
    </>
  )
}

function parseCellPreview(content: string, schema: string): ParsedCellPreview | undefined {
  let cell: Cell
  try {
    cell = Cell.fromBase64(content)
  } catch (error) {
    return {
      title: "TON Cell",
      hash: "",
      bits: 0,
      refs: 0,
      fields: [
        {
          label: "payload",
          value: `${content.length} base64 chars`,
          depth: 0,
        },
      ],
      error: getErrorMessage(error, "Failed to parse cell payload."),
    }
  }

  try {
    const fields: ParsedCellField[] = []
    const runtime = parseTLB(schema)
    const result = runtime.deserialize(content, true)
    const error = result.success ? undefined : result.error.message

    if (result.success) {
      appendParsedCellFields(fields, result.value, "value", 0)
    } else {
      fields.push({
        label: "payload",
        value: `${content.length} base64 chars`,
        depth: 0,
      })
    }

    return {
      title: result.success ? getParsedCellTitle(result.value) : "TON Cell",
      hash: cell.hash().toString("hex"),
      bits: cell.bits.length,
      refs: cell.refs.length,
      fields,
      error,
    }
  } catch (error) {
    return {
      title: "TON Cell",
      hash: cell.hash().toString("hex"),
      bits: cell.bits.length,
      refs: cell.refs.length,
      fields: [
        {
          label: "payload",
          value: `${content.length} base64 chars`,
          depth: 0,
        },
      ],
      error: getErrorMessage(error, "Failed to parse cell payload."),
    }
  }
}

function appendParsedCellFields(
  fields: ParsedCellField[],
  value: ParsedCell,
  label: string,
  depth: number,
): void {
  if (Array.isArray(value)) {
    fields.push({label, value: `Array(${value.length})`, depth, tone: "muted"})
    value.forEach((item, index) => appendParsedCellFields(fields, item, `[${index}]`, depth + 1))
    return
  }

  if (value instanceof Cell) {
    fields.push({
      label,
      value: `Cell ${shortenHash(value.hash().toString("hex"))} (${value.bits.length} bits, ${value.refs.length} refs)`,
      depth,
      tone: "muted",
    })
    return
  }

  if (value && typeof value === "object") {
    const record = value as Readonly<Record<string, ParsedCell>>
    const kind = typeof record.kind === "string" ? record.kind : "Object"
    fields.push({label: depth === 0 ? "kind" : label, value: kind, depth, tone: "muted"})

    for (const [key, nestedValue] of Object.entries(record)) {
      if (key === "kind" || nestedValue === undefined) {
        continue
      }
      appendParsedCellFields(fields, nestedValue, key, depth + 1)
    }
    return
  }

  fields.push({
    label,
    value: formatParsedScalar(label, value),
    depth,
  })
}

function getParsedCellTitle(value: ParsedCell): string {
  if (value && typeof value === "object" && !Array.isArray(value) && !(value instanceof Cell)) {
    const kind = (value as Readonly<Record<string, ParsedCell>>).kind
    if (typeof kind === "string") {
      return kind
    }
  }

  return "TON Cell"
}

function formatParsedScalar(label: string, value: ParsedCell): string {
  if (typeof value === "bigint") {
    if (looksLikeAmount(label)) {
      return `${formatUnits(value.toString(), 9)} GRAM (${value.toString()} nano)`
    }

    if (looksLikeHash(label)) {
      return `0x${value.toString(16).padStart(64, "0")}`
    }

    return `${value.toString()} (0x${value.toString(16)})`
  }

  if (typeof value === "number") {
    if (/flags?/i.test(label) && Number.isInteger(value) && value >= 0 && value <= 255) {
      return `${value} (0b${value.toString(2).padStart(8, "0")})`
    }

    if (Number.isInteger(value) && value > 255) {
      return `${value} (0x${value.toString(16)})`
    }

    return value.toString()
  }

  if (typeof value === "boolean") {
    return value ? "true" : "false"
  }

  if (typeof value === "string") {
    return value
  }

  if (value === null) {
    return "null"
  }

  return String(value)
}

function looksLikeAmount(label: string): boolean {
  return /amount|coins?|grams?|value/i.test(label)
}

function looksLikeHash(label: string): boolean {
  return /hash|digest/i.test(label)
}

function shortenHash(hash: string): string {
  if (hash.length <= 20) {
    return hash
  }

  return `${hash.slice(0, 10)}...${hash.slice(-10)}`
}

function getErrorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message.length > 0 ? error.message : fallback
}

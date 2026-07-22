import {HighlightedCode, ParsedValueView, RawDataBlock, type ParsedValue} from "@acton/ui"
import {Box} from "lucide-react"
import {parseTLB, replacer, type ParsedCell} from "@ton-community/tlb-runtime"
import {Address, Cell, fromNano} from "@ton/core"
import {useMemo} from "react"

import styles from "./TlbCellViewer.module.css"

export interface TlbCellViewerProps {
  readonly boc: string
  readonly schema: string
  readonly defaultExpanded?: boolean
  readonly defaultSchemaExpanded?: boolean
}

interface ParsedTlbCell {
  readonly title: string
  readonly bocHex: string
  readonly bits: number
  readonly refs: number
  readonly value: ParsedValue
  readonly error?: string
}

export function TlbCellViewer({
  boc,
  schema,
  defaultExpanded = true,
  defaultSchemaExpanded = false,
}: TlbCellViewerProps) {
  const parsed = useMemo(() => parseTlbCell(boc, schema), [boc, schema])

  return (
    <div className={styles.root}>
      <div className={styles.summary}>
        <Box size={16} aria-hidden="true" />
        <div className={styles.summaryTitle}>
          <span className={styles.summaryType}>{parsed.title}</span>
          <span className={styles.summaryMetadata}>
            · {parsed.bits} bits · {parsed.refs} refs
          </span>
        </div>
      </div>

      <RawDataBlock
        className={styles.parsedBlock}
        contentClassName={styles.parsedBlockContent}
        title="Parsed Cell"
        titleLabel="parsed cell"
        value={boc}
        copyValue={parsed.bocHex}
        copyLabel="cell BoC as hex"
        collapsible
        defaultExpanded={defaultExpanded}
        customContent={
          <>
            {parsed.error && <div className={styles.error}>{parsed.error}</div>}
            <div className={styles.parsedValue}>
              <ParsedValueView value={parsed.value} />
            </div>
          </>
        }
      />

      <RawDataBlock
        className={styles.schemaBlock}
        title="Schema"
        value={schema}
        copyLabel="TL-B schema"
        collapsible
        defaultExpanded={defaultSchemaExpanded}
        customContent={
          <HighlightedCode
            className={styles.schemaCode}
            value={schema || "No schema"}
            language="tlb"
            wrap
          />
        }
      />
    </div>
  )
}

function parseTlbCell(boc: string, schema: string): ParsedTlbCell {
  let cell: Cell
  try {
    cell = Cell.fromBase64(boc)
  } catch (error) {
    return {
      title: "TON Cell",
      bocHex: "",
      bits: 0,
      refs: 0,
      value: payloadFallback(boc),
      error: getErrorMessage(error, "Failed to parse cell payload."),
    }
  }

  const bocHex = base64ToHex(boc)

  try {
    const result = parseTLB(schema).deserialize(boc, true)
    if (!result.success) {
      return {
        title: "TON Cell",
        bocHex,
        bits: cell.bits.length,
        refs: cell.refs.length,
        value: payloadFallback(boc),
        error: result.error.message,
      }
    }

    return {
      title: getParsedCellTitle(result.value),
      bocHex,
      bits: cell.bits.length,
      refs: cell.refs.length,
      value: toParsedValue(result.value, "value"),
    }
  } catch (error) {
    return {
      title: "TON Cell",
      bocHex,
      bits: cell.bits.length,
      refs: cell.refs.length,
      value: payloadFallback(boc),
      error: getErrorMessage(error, "Failed to parse cell payload."),
    }
  }
}

function toParsedValue(value: ParsedCell, fieldName: string): ParsedValue {
  if (Array.isArray(value)) {
    return {
      kind: "array",
      items: value.map((item, index) => toParsedValue(item, `[${index}]`)),
    }
  }

  if (value && typeof value === "object") {
    const normalized = replacer(fieldName, value)
    if (typeof normalized === "string") {
      const cell = tryParseCell(normalized)
      if (cell) {
        return {
          kind: "scalar",
          typeName: "Cell",
          value: `Cell ${shortenHash(cell.hash().toString("hex"))} (${cell.bits.length} bits, ${cell.refs.length} refs)`,
          rawValue: cell.toBoc().toString("hex"),
        }
      }

      const address = tryParseAddress(normalized)
      if (address) return {kind: "address", value: address.toString()}

      return {kind: "scalar", value: normalized}
    }

    const record = value as Readonly<Record<string, ParsedCell>>
    const typeName = typeof record.kind === "string" ? normalizeTypeName(record.kind) : undefined
    return {
      kind: "object",
      typeName,
      entries: Object.entries(record)
        .filter(([key, nestedValue]) => key !== "kind" && nestedValue !== undefined)
        .map(([key, nestedValue]) => ({key, value: toParsedValue(nestedValue, key)})),
    }
  }

  if (value === null) {
    return {kind: "null"}
  }

  if (typeof value === "boolean") {
    return {kind: "boolean", value}
  }

  return {kind: "scalar", value: formatScalar(fieldName, value)}
}

function formatScalar(fieldName: string, value: ParsedCell): string {
  if (typeof value === "bigint") {
    if (looksLikeAmount(fieldName)) {
      return `${fromNano(value)} GRAM (${value.toString()} nano)`
    }

    if (looksLikeHash(fieldName)) {
      return `0x${value.toString(16).padStart(64, "0")}`
    }

    return `${value.toString()} (0x${value.toString(16)})`
  }

  if (typeof value === "number") {
    if (/flags?/i.test(fieldName) && Number.isInteger(value) && value >= 0 && value <= 255) {
      return `${value} (0b${value.toString(2).padStart(8, "0")})`
    }

    if (Number.isInteger(value) && value > 255) {
      return `${value} (0x${value.toString(16)})`
    }
  }

  return String(value)
}

function getParsedCellTitle(value: ParsedCell): string {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    const kind = (value as Readonly<Record<string, ParsedCell>>).kind
    if (typeof kind === "string") return kind
  }

  return "TON Cell"
}

function normalizeTypeName(typeName: string): string {
  switch (typeName) {
    case "Maybe_just":
      return "Maybe Just"
    case "Maybe_nothing":
    case "none":
      return "Maybe None"
    default:
      return typeName
  }
}

function payloadFallback(boc: string): ParsedValue {
  return {
    kind: "scalar",
    value: `${boc.length} base64 chars`,
    rawValue: boc,
  }
}

function base64ToHex(value: string): string {
  const normalized = value.replaceAll("-", "+").replaceAll("_", "/")
  const binary = atob(normalized.padEnd(Math.ceil(normalized.length / 4) * 4, "="))
  let hex = ""

  for (let index = 0; index < binary.length; index += 1) {
    hex += binary.charCodeAt(index).toString(16).padStart(2, "0")
  }

  return hex
}

function looksLikeAmount(fieldName: string): boolean {
  return /amount|coins?|grams?|value/i.test(fieldName)
}

function looksLikeHash(fieldName: string): boolean {
  return /hash|digest/i.test(fieldName)
}

function shortenHash(hash: string): string {
  return hash.length <= 20 ? hash : `${hash.slice(0, 10)}…${hash.slice(-10)}`
}

function getErrorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message.length > 0 ? error.message : fallback
}

function tryParseCell(value: string): Cell | undefined {
  try {
    return Cell.fromBase64(value)
  } catch {
    return undefined
  }
}

function tryParseAddress(value: string): Address | undefined {
  try {
    return Address.parse(value)
  } catch {
    return undefined
  }
}

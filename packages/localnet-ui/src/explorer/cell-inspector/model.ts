import type {Cell} from "@ton/core"

export type SerializablePrimitive = string | number | boolean | null
export type SerializableObject = {readonly [key: string]: SerializableValue}
export type SerializableValue =
  | SerializablePrimitive
  | SerializableObject
  | readonly SerializableValue[]

export type NormalizedInputKind = "base64" | "hex"
export type NormalizedInputSource = "direct" | "link" | "embedded"

export interface NormalizedCellInput {
  readonly original: string
  readonly value: string
  readonly kind: NormalizedInputKind
  readonly source: NormalizedInputSource
}

export type CellInputErrorCode =
  | "empty-input"
  | "input-too-large"
  | "invalid-format"
  | "invalid-boc"
  | "inspection-failed"
  | "too-many-roots"
  | "root-out-of-range"

export interface CellInputError {
  readonly code: CellInputErrorCode
  readonly message: string
  readonly cause?: string
}

export type NormalizeCellInputResult =
  | {readonly ok: true; readonly input: NormalizedCellInput}
  | {readonly ok: false; readonly error: CellInputError}

export interface DecodedCellInput {
  readonly normalized: NormalizedCellInput
  readonly roots: readonly Cell[]
  readonly selectedRoot: Cell
  readonly selectedRootIndex: number
  readonly byteLength: number
}

export type DecodeCellInputResult =
  | {readonly ok: true; readonly decoded: DecodedCellInput}
  | {readonly ok: false; readonly error: CellInputError; readonly normalized?: NormalizedCellInput}

export type ParserEngine =
  | "abi-registry"
  | "custom-tlb"
  | "standard-comment"
  | "block-tlb"
  | "tvm-disassembly"
  | "raw-cell-tree"

export type ParserConfidenceLevel = "exact" | "high" | "medium" | "low"

export interface ParserConfidence {
  /** A normalized confidence score in the inclusive 0...1 range. */
  readonly score: number
  readonly level: ParserConfidenceLevel
  readonly reasons?: readonly string[]
}

export type ParserProvenanceSource =
  | "abi-registry"
  | "user-schema"
  | "ton-standard"
  | "canonical-block-tlb"
  | "fallback"

export interface ParserProvenance {
  readonly engine: ParserEngine
  readonly label: string
  readonly source: ParserProvenanceSource
  readonly confidence: ParserConfidence
  readonly details?: Readonly<Record<string, string>>
}

export type ParserWarningCode =
  | "partial-match"
  | "ambiguous-match"
  | "abi-context-unavailable"
  | "decryption-key-required"
  | "invalid-utf8"
  | "non-byte-aligned-payload"
  | "snake-ref-limit"
  | "snake-size-limit"
  | "unexpected-snake-refs"
  | "custom-tlb-partial-match"
  | "custom-tlb-error"
  | "tree-depth-limit"
  | "tree-node-limit"
  | "tree-boc-limit"
  | "duplicate-cell"

export interface ParserWarning {
  readonly code: ParserWarningCode
  readonly message: string
  readonly path?: string
}

export interface CellSummary {
  readonly bits: number
  readonly refs: number
  readonly depth: number
  readonly hash: string
  readonly rootIndex: number
  readonly rootCount: number
}

interface CellParseResultBase {
  readonly warnings: readonly ParserWarning[]
  readonly normalized?: NormalizedCellInput
  readonly cell?: CellSummary
  readonly raw?: SerializableValue
}

export type CellParseResult =
  | (CellParseResultBase & {
      readonly status: "success" | "partial"
      readonly parser: ParserEngine
      readonly data: SerializableValue
      readonly provenance: ParserProvenance
    })
  | (CellParseResultBase & {
      readonly status: "unknown"
      readonly parser: "raw-cell-tree"
      readonly data: SerializableValue
      readonly provenance: ParserProvenance
    })
  | (CellParseResultBase & {
      readonly status: "error"
      readonly error: CellInputError
    })

export const confidence = (score: number, reasons?: readonly string[]): ParserConfidence => {
  const boundedScore = Math.min(1, Math.max(0, score))
  const level: ParserConfidenceLevel =
    boundedScore >= 0.99
      ? "exact"
      : boundedScore >= 0.85
        ? "high"
        : boundedScore >= 0.6
          ? "medium"
          : "low"

  return reasons === undefined
    ? {score: boundedScore, level}
    : {score: boundedScore, level, reasons}
}

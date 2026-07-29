import type {Cell, Slice} from "@ton/core"
import {Buffer} from "buffer"

import {confidence, type ParserProvenance, type ParserWarning} from "./model"

export const TEXT_COMMENT_OPCODE = 0x00_00_00_00
export const ENCRYPTED_COMMENT_OPCODE = 0x21_67_da_4b

export interface StandardCommentOptions {
  readonly maxSnakeCells?: number
  readonly maxPayloadBytes?: number
  readonly path?: string
}

interface StandardCommentBase {
  readonly opcode: string
  readonly payloadHex: string
  readonly payloadBytes: number
  readonly provenance: ParserProvenance
  readonly warnings: readonly ParserWarning[]
}

export interface TextComment extends StandardCommentBase {
  readonly kind: "text-comment"
  readonly text?: string
}

export interface EncryptedComment extends StandardCommentBase {
  readonly kind: "encrypted-comment"
  readonly encrypted: true
}

export type StandardComment = TextComment | EncryptedComment

interface SnakeBytes {
  readonly bytes: Buffer
  readonly warnings: readonly ParserWarning[]
}

const DEFAULT_MAX_SNAKE_CELLS = 64
const DEFAULT_MAX_PAYLOAD_BYTES = 1024 * 1024

export function recognizeStandardComment(
  cell: Cell,
  options: StandardCommentOptions = {},
): StandardComment | undefined {
  let slice: Slice
  try {
    slice = cell.beginParse()
  } catch {
    return undefined
  }

  if (slice.remainingBits < 32) {
    return undefined
  }

  const opcode = slice.loadUint(32)
  if (opcode !== TEXT_COMMENT_OPCODE && opcode !== ENCRYPTED_COMMENT_OPCODE) {
    return undefined
  }

  const payload = readSnakeBytes(slice, options)
  const common = {
    opcode: opcodeHex(opcode),
    payloadHex: payload.bytes.toString("hex"),
    payloadBytes: payload.bytes.length,
  }

  if (opcode === ENCRYPTED_COMMENT_OPCODE) {
    const warnings = [
      ...payload.warnings,
      {
        code: "decryption-key-required",
        message: "This encrypted comment requires the recipient's private key to decrypt",
        ...(options.path === undefined ? {} : {path: options.path}),
      } satisfies ParserWarning,
    ]
    return {
      ...common,
      kind: "encrypted-comment",
      encrypted: true,
      provenance: commentProvenance("Encrypted comment", [
        "The standard 0x2167da4b opcode matched exactly",
      ]),
      warnings,
    }
  }

  const warnings = [...payload.warnings]
  let text: string | undefined
  try {
    text = new TextDecoder("utf-8", {fatal: true}).decode(payload.bytes)
  } catch {
    warnings.push({
      code: "invalid-utf8",
      message: "Text comment payload is not valid UTF-8",
      ...(options.path === undefined ? {} : {path: options.path}),
    })
  }

  return {
    ...common,
    kind: "text-comment",
    ...(text === undefined ? {} : {text}),
    provenance: commentProvenance("Text comment", ["The standard zero opcode matched exactly"]),
    warnings,
  }
}

function readSnakeBytes(slice: Slice, options: StandardCommentOptions): SnakeBytes {
  const maxSnakeCells = boundedPositiveInteger(options.maxSnakeCells, DEFAULT_MAX_SNAKE_CELLS)
  const maxPayloadBytes = boundedPositiveInteger(options.maxPayloadBytes, DEFAULT_MAX_PAYLOAD_BYTES)
  const chunks: Buffer[] = []
  const warnings: ParserWarning[] = []
  let current = slice
  let cellsRead = 0
  let bytesRead = 0

  while (true) {
    cellsRead += 1
    if (current.remainingBits % 8 !== 0) {
      warnings.push(
        withPath(options, {
          code: "non-byte-aligned-payload",
          message: `Comment payload has ${current.remainingBits % 8} trailing bit(s) outside complete bytes`,
        }),
      )
    }

    const availableBytes = Math.floor(current.remainingBits / 8)
    const remainingBudget = Math.max(0, maxPayloadBytes - bytesRead)
    const bytesToRead = Math.min(availableBytes, remainingBudget)
    if (bytesToRead > 0) {
      chunks.push(current.loadBuffer(bytesToRead))
      bytesRead += bytesToRead
    }

    if (availableBytes > remainingBudget) {
      warnings.push(
        withPath(options, {
          code: "snake-size-limit",
          message: `Comment payload was truncated at ${maxPayloadBytes.toLocaleString()} bytes`,
        }),
      )
      break
    }

    if (current.remainingRefs === 0) {
      break
    }

    if (current.remainingRefs !== 1) {
      warnings.push(
        withPath(options, {
          code: "unexpected-snake-refs",
          message: `A comment cell contains ${current.remainingRefs} references; only the first was followed`,
        }),
      )
    }

    if (cellsRead >= maxSnakeCells) {
      warnings.push(
        withPath(options, {
          code: "snake-ref-limit",
          message: `Comment payload was truncated after ${maxSnakeCells} snake cell(s)`,
        }),
      )
      break
    }

    current = current.loadRef().beginParse(true)
  }

  return {bytes: Buffer.concat(chunks), warnings}
}

function commentProvenance(label: string, reasons: readonly string[]): ParserProvenance {
  return {
    engine: "standard-comment",
    label,
    source: "ton-standard",
    confidence: confidence(1, reasons),
  }
}

function opcodeHex(opcode: number): string {
  return `0x${opcode.toString(16).padStart(8, "0")}`
}

function boundedPositiveInteger(value: number | undefined, fallback: number): number {
  return value === undefined || !Number.isFinite(value) ? fallback : Math.max(1, Math.trunc(value))
}

function withPath(
  options: StandardCommentOptions,
  warning: Omit<ParserWarning, "path">,
): ParserWarning {
  return options.path === undefined ? warning : {...warning, path: options.path}
}

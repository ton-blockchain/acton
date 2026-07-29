import {Cell} from "@ton/core"

import type {
  CellInputError,
  DecodeCellInputResult,
  DecodedCellInput,
  NormalizeCellInputResult,
  NormalizedCellInput,
  NormalizedInputSource,
} from "./model"

export const DEFAULT_MAX_INPUT_BYTES = 4 * 1024 * 1024
export const DEFAULT_MAX_ROOTS = 128

const DEFAULT_MAX_ENCODED_CHARS = DEFAULT_MAX_INPUT_BYTES * 2 + 4096
const WHITESPACE = /\s+/g
const HEX = /^[0-9a-f]+$/i
const BASE64 = /^[A-Za-z0-9+/_-]+={0,2}$/
const EMBEDDED_HEX_BOC = /(?:b5ee9c72|68ff65f3|acc3a728)[0-9a-f]{8,}/gi
const EMBEDDED_BASE64_BOC = /(?:te6cc|aP9l8|rMOnK)[A-Za-z0-9+/_=-]{8,}/g
const BOC_MAGIC = new Set(["b5ee9c72", "68ff65f3", "acc3a728"])
const EXPLORER_KEYS = [
  "boc",
  "cell",
  "data",
  "payload",
  "message",
  "msg",
  "state",
  "proof",
  "value",
] as const

export interface NormalizeCellInputOptions {
  readonly maxEncodedChars?: number
}

export interface DecodeCellInputOptions extends NormalizeCellInputOptions {
  readonly maxInputBytes?: number
  readonly maxRoots?: number
  readonly rootIndex?: number
}

export function normalizeCellInput(
  rawInput: string,
  options: NormalizeCellInputOptions = {},
): NormalizeCellInputResult {
  const maxEncodedChars = options.maxEncodedChars ?? DEFAULT_MAX_ENCODED_CHARS
  if (rawInput.length > maxEncodedChars) {
    return normalizeFailure(
      "input-too-large",
      `Cell input exceeds the ${maxEncodedChars.toLocaleString()} character limit`,
    )
  }

  const trimmed = rawInput.trim()
  if (trimmed.length === 0) {
    return normalizeFailure("empty-input", "Enter a cell or BoC to inspect")
  }

  const direct = normalizeCandidate(trimmed, rawInput, "direct")
  if (direct !== undefined) {
    return {ok: true, input: direct}
  }

  for (const candidate of collectLinkCandidates(trimmed)) {
    const normalized = normalizeCandidate(candidate, rawInput, "link")
    if (normalized !== undefined) {
      return {ok: true, input: normalized}
    }
  }

  for (const candidate of collectEmbeddedCandidates(trimmed)) {
    const normalized = normalizeCandidate(candidate, rawInput, "embedded")
    if (normalized !== undefined) {
      return {ok: true, input: normalized}
    }
  }

  return normalizeFailure(
    "invalid-format",
    "No valid cell or BoC found. Paste Base64, hex, or a link containing one",
  )
}

export function decodeCellInput(
  rawInput: string,
  options: DecodeCellInputOptions = {},
): DecodeCellInputResult {
  const normalizedResult = normalizeCellInput(rawInput, options)
  if (!normalizedResult.ok) {
    return normalizedResult
  }

  const {input} = normalizedResult
  const bytes =
    input.kind === "hex" ? Buffer.from(input.value, "hex") : Buffer.from(input.value, "base64")
  const maxInputBytes = options.maxInputBytes ?? DEFAULT_MAX_INPUT_BYTES

  if (bytes.length > maxInputBytes) {
    return decodeFailure(
      "input-too-large",
      `Decoded BoC exceeds the ${maxInputBytes.toLocaleString()} byte limit`,
      input,
    )
  }

  let roots: Cell[]
  try {
    roots = Cell.fromBoc(bytes)
  } catch (error) {
    return decodeFailure(
      "invalid-boc",
      "The input starts like a BoC, but its contents are invalid",
      input,
      errorMessage(error),
    )
  }

  if (roots.length === 0) {
    return decodeFailure("invalid-boc", "The BoC does not contain any root cells", input)
  }

  const maxRoots = options.maxRoots ?? DEFAULT_MAX_ROOTS
  if (roots.length > maxRoots) {
    return decodeFailure(
      "too-many-roots",
      `The BoC contains ${roots.length.toLocaleString()} roots; the limit is ${maxRoots.toLocaleString()}`,
      input,
    )
  }

  const rootIndex = options.rootIndex ?? 0
  if (!Number.isSafeInteger(rootIndex) || rootIndex < 0 || rootIndex >= roots.length) {
    return decodeFailure(
      "root-out-of-range",
      `Root ${String(rootIndex)} is unavailable. This BoC contains ${roots.length} ${roots.length === 1 ? "root cell" : "root cells"}`,
      input,
    )
  }

  const selectedRoot = roots[rootIndex]
  if (selectedRoot === undefined) {
    return decodeFailure("root-out-of-range", `Root ${rootIndex} is unavailable`, input)
  }

  const decoded: DecodedCellInput = {
    normalized: input,
    roots,
    selectedRoot,
    selectedRootIndex: rootIndex,
    byteLength: bytes.length,
  }
  return {ok: true, decoded}
}

export function canonicalizeBase64(value: string): string {
  const normalized = value
    .replace(WHITESPACE, "")
    .replace(/-/g, "+")
    .replace(/_/g, "/")
    .replace(/[=]+$/, "")
  const remainder = normalized.length % 4
  return remainder === 0 ? normalized : `${normalized}${"=".repeat(4 - remainder)}`
}

function normalizeCandidate(
  candidate: string,
  original: string,
  source: NormalizedInputSource,
): NormalizedCellInput | undefined {
  const cleaned = cleanCandidate(candidate)
  const hex = cleaned.replace(/^0x/i, "")

  if (hex.length % 2 === 0 && HEX.test(hex)) {
    const bytes = Buffer.from(hex, "hex")
    if (hasBocMagic(bytes)) {
      return {original, value: hex.toLowerCase(), kind: "hex", source}
    }
  }

  if (!isValidBase64(cleaned)) {
    return undefined
  }

  const canonical = canonicalizeBase64(cleaned)
  const bytes = Buffer.from(canonical, "base64")
  return hasBocMagic(bytes) ? {original, value: canonical, kind: "base64", source} : undefined
}

function cleanCandidate(candidate: string): string {
  let cleaned = decodeSafely(candidate)
    .trim()
    .replace(/^['"`]+|['"`]+$/g, "")
  cleaned = cleaned
    .replace(/^ton:\/\/(?:cell|boc)\//i, "")
    .replace(/^cell:\/\//i, "")
    .replace(/^boc:/i, "")
  return decodeSafely(cleaned).replace(WHITESPACE, "")
}

function isValidBase64(value: string): boolean {
  if (!BASE64.test(value)) {
    return false
  }

  const firstPadding = value.indexOf("=")
  const bodyLength = firstPadding === -1 ? value.length : firstPadding
  return bodyLength % 4 !== 1
}

function hasBocMagic(bytes: Uint8Array): boolean {
  return bytes.length >= 4 && BOC_MAGIC.has(Buffer.from(bytes.subarray(0, 4)).toString("hex"))
}

function collectLinkCandidates(input: string): string[] {
  const candidates: string[] = []

  for (const urlCandidate of unique([input, decodeSafely(input)])) {
    try {
      const url = new URL(urlCandidate)
      for (const key of EXPLORER_KEYS) {
        const value = url.searchParams.get(key)
        if (value !== null) {
          candidates.push(value)
        }
      }
      candidates.push(...url.searchParams.values())

      for (const segment of url.pathname.split("/")) {
        if (segment.length > 0) {
          candidates.push(segment)
        }
      }

      if (url.hash.length > 1) {
        candidates.push(url.hash.slice(1))
      }
    } catch {
      // A plain BoC or pasted text is handled by the other candidate collectors.
    }
  }

  return unique(candidates)
}

function collectEmbeddedCandidates(input: string): string[] {
  const decoded = decodeSafely(input)
  return unique([
    ...(decoded.match(EMBEDDED_HEX_BOC) ?? []),
    ...(decoded.match(EMBEDDED_BASE64_BOC) ?? []),
  ])
}

function decodeSafely(value: string): string {
  try {
    return decodeURIComponent(value)
  } catch {
    return value
  }
}

function unique(values: readonly string[]): string[] {
  return [...new Set(values)]
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function normalizeFailure(code: CellInputError["code"], message: string): NormalizeCellInputResult {
  return {ok: false, error: {code, message}}
}

function decodeFailure(
  code: CellInputError["code"],
  message: string,
  normalized: NormalizedCellInput,
  cause?: string,
): DecodeCellInputResult {
  const error: CellInputError = cause === undefined ? {code, message} : {code, message, cause}
  return {ok: false, error, normalized}
}

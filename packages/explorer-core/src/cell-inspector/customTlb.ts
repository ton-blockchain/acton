import {parseTLB} from "@ton-community/tlb-runtime"
import type {Cell} from "@ton/core"
import {formatNumberValue} from "@acton/ui"

import {
  confidence,
  type ParserProvenance,
  type ParserWarning,
  type SerializableValue,
} from "./model"
import {toSerializable} from "./serializable"

export interface CustomTlbOptions {
  readonly findByTag?: boolean
  readonly maxSchemaChars?: number
}

export type CustomTlbParseResult =
  | {
      readonly matched: true
      readonly data: SerializableValue
      readonly provenance: ParserProvenance
      readonly warnings: readonly ParserWarning[]
    }
  | {
      readonly matched: false
      readonly error: string
      readonly warnings: readonly ParserWarning[]
    }

const DEFAULT_MAX_SCHEMA_CHARS = 250_000
const MAX_CACHED_SCHEMAS = 12
const runtimeCache = new Map<string, ReturnType<typeof parseTLB>>()

/**
 * Parses a cell with a user-provided TL-B schema. The runtime chooses the schema's final type,
 * unless `findByTag` is enabled. A blank schema intentionally skips this parser.
 */
export function tryParseCustomTlb(
  cell: Cell,
  schema: string,
  options: CustomTlbOptions = {},
): CustomTlbParseResult | undefined {
  const trimmedSchema = schema.trim()
  if (trimmedSchema.length === 0) {
    return undefined
  }

  const maxSchemaChars = options.maxSchemaChars ?? DEFAULT_MAX_SCHEMA_CHARS
  if (trimmedSchema.length > maxSchemaChars) {
    const message = `TL-B schema exceeds the ${formatNumberValue(maxSchemaChars)} character limit`
    return customTlbFailure(message)
  }

  try {
    const runtime = getCachedRuntime(trimmedSchema)
    // Passing a string avoids nominal Cell type differences between the app and runtime's
    // nested @ton/core version while preserving the exact selected cell.
    const result = runtime.deserialize(
      cell.toBoc({idx: false, crc32: false}).toString("base64"),
      options.findByTag ?? false,
    )
    if (!result.success) {
      return customTlbFailure(result.error.message)
    }

    const warnings: ParserWarning[] = [
      {
        code: "custom-tlb-partial-match",
        message:
          "This schema decoded the root, but the decoder cannot verify whether all bits and references were used",
      },
    ]
    return {
      matched: true,
      data: toSerializable(result.value),
      provenance: {
        engine: "custom-tlb",
        label: "Custom TL-B schema",
        source: "user-schema",
        confidence: confidence(0.82, [
          "The custom schema decoded the root successfully",
          "The decoder cannot measure unread data for custom schemas",
        ]),
        details: {schemaCharacters: String(trimmedSchema.length)},
      },
      warnings,
    }
  } catch (error) {
    return customTlbFailure(errorMessage(error))
  }
}

export function clearCustomTlbCache(): void {
  runtimeCache.clear()
}

function getCachedRuntime(schema: string): ReturnType<typeof parseTLB> {
  const cached = runtimeCache.get(schema)
  if (cached !== undefined) {
    runtimeCache.delete(schema)
    runtimeCache.set(schema, cached)
    return cached
  }

  const runtime = parseTLB(schema)
  if (runtimeCache.size >= MAX_CACHED_SCHEMAS) {
    const oldest = runtimeCache.keys().next().value
    if (oldest !== undefined) {
      runtimeCache.delete(oldest)
    }
  }
  runtimeCache.set(schema, runtime)
  return runtime
}

function customTlbFailure(message: string): CustomTlbParseResult {
  return {
    matched: false,
    error: message,
    warnings: [
      {code: "custom-tlb-error", message: `Custom TL-B could not decode this root: ${message}`},
    ],
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

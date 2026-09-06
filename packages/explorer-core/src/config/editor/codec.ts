import {beginCell, Cell, Dictionary} from "@ton/core"

import {
  loadConfigParam,
  storeConfigParam,
  type ConfigParam,
} from "../../cell-inspector/block.tlb.generated"
import schema from "./schema.generated.json"
import {
  decimalValue,
  fieldSemantics,
  formatConfigAddress,
  parseConfigAddress,
  scaledInteger,
  type FieldContext,
} from "./semantics"

/** Serializable form state retains incomplete numeric input without truncation. */
export type Draft = string | boolean | null | Draft[] | {[field: string]: Draft}

export type Shape =
  | {type: "ref"; name: string}
  | {type: "struct"; fields: Record<string, Shape>}
  | {type: "union"; options: Shape[]}
  | {type: "map"; key: Shape; value: Shape}
  | {type: "maybe"; value: Shape}
  | {type: "literal"; value: string}
  | {
      type:
        | "number"
        | "bigint"
        | "boolean"
        | "undefined"
        | "buffer"
        | "cell"
        | "slice"
        | "bitstring"
    }

const definitions = schema.definitions as unknown as Record<string, Shape>
const parameters = schema.parameters as Record<string, string>

export const editableParameterIds = Object.keys(parameters).map(Number)

export function parameterShape(index: number): Shape | undefined {
  const name = parameters[index]
  return name ? {type: "ref", name} : undefined
}

export function resolveShape(shape: Shape): Shape {
  if (shape.type !== "ref") return shape
  const definition = definitions[shape.name]
  if (!definition) throw new Error(`Missing editor schema: ${shape.name}`)
  return resolveShape(definition)
}

export function objectDraft(value: Draft): Record<string, Draft> {
  if (value === null || Array.isArray(value) || typeof value !== "object")
    throw new Error("Expected a structured parameter")
  return value
}

export function selectedShape(shape: Shape, value: Draft): Shape {
  const resolved = resolveShape(shape)
  if (resolved.type !== "union") return shape
  const first = resolved.options[0]
  if (!first) throw new Error("No available configuration formats")
  return (
    resolved.options.find(option => {
      const candidate = resolveShape(option)
      return (
        candidate.type === "struct" &&
        candidate.fields.kind?.type === "literal" &&
        value !== null &&
        typeof value === "object" &&
        !Array.isArray(value) &&
        candidate.fields.kind.value === value.kind
      )
    }) ?? first
  )
}

export function defaultDraft(shape: Shape, depth = 0): Draft {
  if (depth > 12) throw new Error("Select a non-recursive format for this value")
  const value = resolveShape(shape)
  switch (value.type) {
    case "struct":
      return Object.fromEntries(
        Object.entries(value.fields).map(([field, nested]) => [
          field,
          defaultDraft(nested, depth + 1),
        ]),
      )
    case "union":
      if (!value.options[0]) throw new Error("No available configuration formats")
      return defaultDraft(value.options[0], depth + 1)
    case "maybe":
    case "undefined":
      return null
    case "map":
      return []
    case "literal":
      return value.value
    case "boolean":
      return false
    default:
      return ""
  }
}

/** Only offer structured editing when decoding and re-encoding preserves every bit. */
export function decodeParameter(index: number, cell: Cell): Draft {
  const shape = parameterShape(index)
  if (!shape) throw new Error("This parameter uses a custom schema; edit its raw BoC")
  const slice = cell.beginParse()
  const parsed = loadConfigParam(slice, index)
  slice.endParse()
  const draft = convert(shape, parsed, {parameter: index, field: ""}, false) as Draft
  if (!encodeParameter(index, draft).equals(cell))
    throw new Error("The structured editor cannot preserve this encoding; use raw BoC")
  return draft
}

export function encodeParameter(index: number, draft: Draft): Cell {
  const shape = parameterShape(index)
  if (!shape) throw new Error("Use raw BoC for this parameter")
  const value = convert(shape, draft, {parameter: index, field: ""}, true) as ConfigParam
  const cell = beginCell().store(storeConfigParam(value)).endCell()
  const slice = cell.beginParse()
  loadConfigParam(slice, index)
  slice.endParse()
  return cell
}

/** Accepts the explorer's hex BoC and standard base64 without guessing cell bits. */
export function parseParameterBoc(text: string): Cell {
  const compact = text.replace(/\s/g, "")
  if (!compact) throw new Error("Enter a parameter BoC in hex or base64")
  if (compact.length > 1_000_000) throw new Error("Enter a BoC smaller than 1 MB")
  const bytes =
    /^[\da-f]+$/i.test(compact) && compact.length % 2 === 0
      ? Buffer.from(compact, "hex")
      : Buffer.from(compact, "base64")
  const cells = Cell.fromBoc(bytes)
  if (cells.length !== 1 || !cells[0] || cells[0].isExotic)
    throw new Error("Use a single ordinary root cell")
  return cells[0]
}

function convert(shape: Shape, value: unknown, context: FieldContext, encode: boolean): unknown {
  const resolved = resolveShape(shape)
  const owner = shape.type === "ref" ? shape.name : context.owner
  const semantics = fieldSemantics(context)
  switch (resolved.type) {
    case "union":
      return convert(selectedShape(shape, value as Draft), value, context, encode)
    case "struct": {
      const record = value as Record<string, unknown>
      return Object.fromEntries(
        Object.entries(resolved.fields).map(([field, nested]) => [
          field,
          convert(
            nested,
            record[field],
            field === "anon0"
              ? {...context, owner}
              : {parameter: context.parameter, field, owner, parentField: context.field},
            encode,
          ),
        ]),
      )
    }
    case "maybe": {
      if (encode)
        return value === null
          ? {kind: "Maybe_nothing"}
          : {kind: "Maybe_just", value: convert(resolved.value, value, context, true)}
      const optional = value as {kind: string; value?: unknown}
      return optional.kind === "Maybe_nothing"
        ? null
        : convert(resolved.value, optional.value, context, false)
    }
    case "map": {
      const keyContext = {...context, mapRole: "key" as const}
      const valueContext = {...context, mapRole: "value" as const}
      if (!encode)
        return [...(value as Dictionary<DictionaryKey, unknown>)].map(([key, entry]) => ({
          key: convert(resolved.key, key, keyContext, false),
          value: convert(resolved.value, entry, {...valueContext, mapKey: String(key)}, false),
        }))
      const map = Dictionary.empty<DictionaryKey, unknown>()
      for (const entry of value as {key: Draft; value: Draft}[]) {
        const key = convert(resolved.key, entry.key, keyContext, true) as DictionaryKey
        if (map.has(key)) throw new Error(`Duplicate entry: ${entry.key}`)
        map.set(
          key,
          convert(resolved.value, entry.value, {...valueContext, mapKey: String(key)}, true),
        )
      }
      return map
    }
    case "literal":
      return resolved.value
    case "undefined":
      return encode ? undefined : null
    case "boolean":
      return value
    case "cell":
    case "slice": {
      if (encode) {
        const cell = parseParameterBoc(String(value))
        return resolved.type === "slice" ? cell.beginParse() : cell
      }
      const cell = value instanceof Cell ? value : (value as {asCell(): Cell}).asCell()
      return cell.toBoc().toString("base64")
    }
    case "buffer": {
      if (semantics.format === "address")
        return encode
          ? Buffer.from(parseConfigAddress(String(value)).toString(16).padStart(64, "0"), "hex")
          : formatConfigAddress(BigInt(`0x${(value as Buffer).toString("hex")}`))
      if (!encode) return (value as Buffer).toString("hex")
      const text = String(value).trim().replace(/^0x/, "")
      if (!/^(?:[\da-f]{2})+$/i.test(text)) throw new Error("Enter complete hexadecimal bytes")
      return Buffer.from(text, "hex")
    }
    case "number":
    case "bigint": {
      if (semantics.format === "float32") {
        const bits = new DataView(new ArrayBuffer(4))
        if (encode) {
          const number = Number(String(value).trim())
          if (String(value).trim() === "" || !Number.isFinite(Math.fround(number)))
            throw new Error("Enter a finite multiplier")
          bits.setFloat32(0, number)
          return bits.getUint32(0)
        }
        bits.setUint32(0, Number(value))
        const number = bits.getFloat32(0)
        if (!Number.isFinite(number)) throw new Error("Non-finite multiplier; use raw BoC")
        // Display the shortest decimal that round-trips to these exact float32 bits.
        for (let precision = 1; precision <= 9; precision++) {
          const text = Number(number.toPrecision(precision)).toString()
          if (Math.fround(Number(text)) === number) return text
        }
        return String(number)
      }
      let integer: bigint
      if (!encode) {
        integer = BigInt(value as number | bigint)
        if (semantics.format === "address" || semantics.format === "wide-address")
          return formatConfigAddress(integer, semantics.format === "wide-address")
        if (semantics.format === "hash") return integer.toString(16).padStart(64, "0")
        if ([9, 10].includes(context.parameter) && context.mapRole === "key")
          integer = BigInt.asIntN(32, integer)
        return decimalValue(integer, semantics.scale ?? 1n)
      }
      const text = String(value).trim()
      if (semantics.format === "address" || semantics.format === "wide-address")
        integer = parseConfigAddress(text, semantics.format === "wide-address")
      else if (semantics.format === "hash") {
        if (!/^(?:0x)?[\da-f]{64}$/i.test(text)) throw new Error("Enter a 256-bit hexadecimal hash")
        integer = BigInt(`0x${text.replace(/^0x/, "")}`)
      } else integer = scaledInteger(text, semantics.scale)
      if ([9, 10].includes(context.parameter) && context.mapRole === "key") {
        if (integer < -2147483648n || integer > 2147483647n)
          throw new Error("Parameter index must fit signed 32 bits")
        integer = BigInt.asUintN(32, integer)
      }
      if (resolved.type === "bigint") return integer
      const number = Number(integer)
      if (!Number.isSafeInteger(number)) throw new Error("Integer is outside the supported range")
      return number
    }
    default:
      throw new Error(`Unsupported editor field: ${resolved.type}`)
  }
}

type DictionaryKey = number | bigint | Buffer

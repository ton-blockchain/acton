import {Address, Cell, Dictionary, ExternalAddress} from "@ton/core"
import {Buffer} from "buffer"

import type {SerializableObject, SerializableValue} from "./model"

interface DictionaryLike {
  keys: () => unknown[]
  get: (key: unknown) => unknown
}

interface CellLike {
  toBoc: () => Uint8Array
}

export function toSerializable(value: unknown): SerializableValue {
  return serializeValue(value, new WeakSet<object>()) ?? null
}

function serializeValue(value: unknown, seen: WeakSet<object>): SerializableValue | undefined {
  if (value === null) {
    return null
  }

  if (typeof value === "string" || typeof value === "boolean") {
    return value
  }

  if (typeof value === "number") {
    return Number.isFinite(value) ? value : String(value)
  }

  if (typeof value === "bigint") {
    return value.toString()
  }

  if (typeof value === "undefined" || typeof value === "function" || typeof value === "symbol") {
    return undefined
  }

  if (typeof value !== "object") {
    return String(value)
  }

  if (Buffer.isBuffer(value)) {
    return value.toString("hex")
  }

  if (value instanceof Uint8Array) {
    return Buffer.from(value).toString("hex")
  }

  if (isCell(value)) {
    return Buffer.from(value.toBoc()).toString("hex")
  }

  if (isAddress(value)) {
    return value.toString()
  }

  if (isBitString(value)) {
    return String(value)
  }

  if (seen.has(value)) {
    return "[Circular]"
  }
  seen.add(value)

  try {
    if (value instanceof Date) {
      return Number.isNaN(value.valueOf()) ? "Invalid Date" : value.toISOString()
    }

    if (value instanceof Error) {
      const cause = serializeValue(value.cause, seen)
      return cause === undefined
        ? {name: value.name, message: value.message}
        : {name: value.name, message: value.message, cause}
    }

    if (value instanceof Map) {
      return serializeEntries(value.entries(), seen)
    }

    if (value instanceof Set) {
      return [...value].map(entry => serializeValue(entry, seen) ?? null)
    }

    if (isDictionary(value)) {
      return serializeEntries(
        value.keys().map(key => [key, value.get(key)] as const),
        seen,
      )
    }

    if (Array.isArray(value)) {
      return value.map(entry => serializeValue(entry, seen) ?? null)
    }

    const result: Record<string, SerializableValue> = {}
    for (const [key, entry] of Object.entries(value)) {
      const serialized = serializeValue(entry, seen)
      if (serialized !== undefined) {
        result[key] = serialized
      }
    }
    return result
  } finally {
    seen.delete(value)
  }
}

function serializeEntries(
  entries: Iterable<readonly [unknown, unknown]>,
  seen: WeakSet<object>,
): SerializableObject {
  const result: Record<string, SerializableValue> = {}
  for (const [key, value] of entries) {
    const serialized = serializeValue(value, seen)
    if (serialized !== undefined) {
      result[String(key)] = serialized
    }
  }
  return result
}

function isCell(value: object): value is CellLike {
  if (value instanceof Cell) {
    return true
  }

  return (
    value.constructor.name === "Cell" &&
    "toBoc" in value &&
    typeof (value as {toBoc?: unknown}).toBoc === "function"
  )
}

function isAddress(value: object): value is Address | ExternalAddress {
  if (value instanceof Address || value instanceof ExternalAddress) {
    return true
  }

  return value.constructor.name === "Address" || value.constructor.name === "ExternalAddress"
}

function isBitString(value: object): boolean {
  return value.constructor.name === "BitString" && typeof value.toString === "function"
}

function isDictionary(value: object): value is DictionaryLike {
  if (value instanceof Dictionary) {
    return true
  }

  return (
    value.constructor.name === "Dictionary" &&
    "keys" in value &&
    typeof (value as {keys?: unknown}).keys === "function" &&
    "get" in value &&
    typeof (value as {get?: unknown}).get === "function"
  )
}

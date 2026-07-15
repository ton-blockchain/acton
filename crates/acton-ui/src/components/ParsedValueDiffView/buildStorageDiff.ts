import type {ParsedValue, ParsedValueLeaf} from "../ParsedValueView/types"

import type {ParsedValueDiff, ParsedValueDiffContainerKind, ParsedValueDiffStatus} from "./types"

export interface ParsedStorageValue {
  readonly name: string
  readonly value: ParsedValue
}

interface StorageValueEntry {
  readonly key: string
  readonly value: StorageValue
}

type StorageValue =
  | ParsedValueLeaf
  | {
      readonly kind: "object"
      readonly objectKind: ParsedValueDiffContainerKind
      readonly typeName?: string
      readonly entries: readonly StorageValueEntry[]
    }

const scalar = (value: string, rawValue?: string, typeName?: string): ParsedValueLeaf => ({
  kind: "scalar",
  value,
  rawValue,
  typeName,
})

const objectValue = (
  entries: readonly StorageValueEntry[],
  typeName?: string,
  objectKind: ParsedValueDiffContainerKind = "object",
): Extract<StorageValue, {readonly kind: "object"}> => ({
  kind: "object",
  objectKind,
  typeName,
  entries,
})

function stringifyParsedValue(value: ParsedValue): string {
  switch (value.kind) {
    case "null":
      return "null"
    case "void":
      return "void"
    case "address":
    case "scalar":
      return value.value
    case "boolean":
      return value.value ? "true" : "false"
    case "array":
      return `[${value.items.map(item => stringifyParsedValue(item)).join(", ")}]`
    case "object": {
      const renderedEntries = value.entries
        .map(entry => `${entry.key}: ${stringifyParsedValue(entry.value)}`)
        .join(", ")
      return value.typeName ? `${value.typeName} { ${renderedEntries} }` : `{ ${renderedEntries} }`
    }
    case "map": {
      const renderedEntries = value.entries
        .map(entry => `${stringifyParsedValue(entry.key)} => ${stringifyParsedValue(entry.value)}`)
        .join(", ")
      return `${value.typeName ?? "map"} { ${renderedEntries} }`
    }
  }
}

function normalizeParsedValue(value: ParsedValue): StorageValue {
  switch (value.kind) {
    case "null":
    case "void":
    case "boolean":
    case "scalar":
    case "address":
      return value
    case "array":
      return objectValue(
        value.items.map((item, index) => ({
          key: `[${index}]`,
          value: normalizeParsedValue(item),
        })),
        "array",
        "array",
      )
    case "map":
      return objectValue(
        value.entries.map(entry => ({
          key: stringifyParsedValue(entry.key),
          value: normalizeParsedValue(entry.value),
        })),
        value.typeName ?? "map",
        "map",
      )
    case "object":
      return objectValue(
        value.entries.map(entry => ({
          key: entry.key,
          value: normalizeParsedValue(entry.value),
        })),
        value.typeName,
      )
  }
}

function normalizeStorage(value: ParsedStorageValue | undefined): StorageValue | undefined {
  if (!value) return undefined

  const normalized = normalizeParsedValue(value.value)
  if (normalized.kind === "object") {
    return normalized.typeName
      ? normalized
      : objectValue(normalized.entries, value.name, normalized.objectKind)
  }

  return objectValue([{key: "value", value: normalized}], value.name)
}

function toAddedDiff(value: StorageValue): ParsedValueDiff {
  if (value.kind === "object") {
    return {
      kind: "object",
      status: "added",
      objectKind: value.objectKind,
      typeName: value.typeName,
      entries: value.entries.map(entry => ({
        key: entry.key,
        value: toAddedDiff(entry.value),
      })),
    }
  }

  return {
    kind: "leaf",
    status: "added",
    before: undefined,
    after: value,
  }
}

function toRemovedDiff(value: StorageValue): ParsedValueDiff {
  if (value.kind === "object") {
    return {
      kind: "object",
      status: "removed",
      objectKind: value.objectKind,
      typeName: value.typeName,
      entries: value.entries.map(entry => ({
        key: entry.key,
        value: toRemovedDiff(entry.value),
      })),
    }
  }

  return {
    kind: "leaf",
    status: "removed",
    before: value,
    after: undefined,
  }
}

function areLeafValuesEqual(before: ParsedValueLeaf, after: ParsedValueLeaf): boolean {
  if (before.kind !== after.kind) return false

  if (before.kind === "null" && after.kind === "null") return true
  if (before.kind === "boolean" && after.kind === "boolean") return before.value === after.value
  if (before.kind === "address" && after.kind === "address") return before.value === after.value

  if (before.kind === "scalar" && after.kind === "scalar") {
    return (before.rawValue ?? before.value) === (after.rawValue ?? after.value)
  }

  return false
}

function diffStorageValues(
  before: StorageValue | undefined,
  after: StorageValue | undefined,
): ParsedValueDiff | undefined {
  if (!before && !after) return undefined
  if (!before) return after ? toAddedDiff(after) : undefined
  if (!after) return toRemovedDiff(before)

  if (before.kind !== after.kind) {
    return {
      kind: "leaf",
      status: "changed",
      before: before.kind === "object" ? scalar(before.typeName ?? "{...}") : before,
      after: after.kind === "object" ? scalar(after.typeName ?? "{...}") : after,
    }
  }

  if (before.kind !== "object" && after.kind !== "object") {
    return {
      kind: "leaf",
      status: areLeafValuesEqual(before, after) ? "unchanged" : "changed",
      before,
      after,
    }
  }

  if (before.kind !== "object" || after.kind !== "object") {
    return {
      kind: "leaf",
      status: "changed",
      before: before.kind === "object" ? scalar(before.typeName ?? "{...}") : before,
      after: after.kind === "object" ? scalar(after.typeName ?? "{...}") : after,
    }
  }

  const orderedKeys = before.entries.map(entry => entry.key)
  const knownKeys = new Set(orderedKeys)
  for (const entry of after.entries) {
    if (knownKeys.has(entry.key)) continue
    orderedKeys.push(entry.key)
    knownKeys.add(entry.key)
  }
  const beforeEntryMap = new Map(before.entries.map(entry => [entry.key, entry.value]))
  const afterEntryMap = new Map(after.entries.map(entry => [entry.key, entry.value]))
  const entries = orderedKeys.flatMap(key => {
    const value = diffStorageValues(beforeEntryMap.get(key), afterEntryMap.get(key))
    return value ? [{key, value}] : []
  })
  const status: ParsedValueDiffStatus =
    before.typeName !== after.typeName ||
    before.objectKind !== after.objectKind ||
    entries.some(entry => entry.value.status !== "unchanged")
      ? "changed"
      : "unchanged"

  return {
    kind: "object",
    status,
    objectKind: after.objectKind,
    typeName: after.typeName ?? before.typeName,
    entries,
  }
}

export function buildStorageDiff(
  before: ParsedStorageValue | undefined,
  after: ParsedStorageValue | undefined,
): ParsedValueDiff | undefined {
  return diffStorageValues(normalizeStorage(before), normalizeStorage(after))
}

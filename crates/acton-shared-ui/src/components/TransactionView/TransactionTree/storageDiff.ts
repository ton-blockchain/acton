import type {ParsedContractStorage, ParsedValue} from "@/types/transaction"

export type StorageDiffStatus = "unchanged" | "changed" | "added" | "removed"

export type StorageLeafValue =
  | {
      readonly kind: "null"
    }
  | {
      readonly kind: "void"
    }
  | {
      readonly kind: "address"
      readonly value: string
    }
  | {
      readonly kind: "scalar"
      readonly value: string
      readonly rawValue?: string
      readonly typeName?: string
    }
  | {
      readonly kind: "boolean"
      readonly value: boolean
    }

export interface StorageValueEntry {
  readonly key: string
  readonly value: StorageValue
}

export type StorageObjectKind = "object" | "array" | "map"

export type StorageValue =
  | StorageLeafValue
  | {
      readonly kind: "object"
      readonly objectKind: StorageObjectKind
      readonly typeName?: string
      readonly entries: readonly StorageValueEntry[]
    }

export interface StorageDiffEntry {
  readonly key: string
  readonly value: StorageDiffNode
}

export type StorageDiffNode =
  | {
      readonly kind: "leaf"
      readonly status: StorageDiffStatus
      readonly before: StorageLeafValue | undefined
      readonly after: StorageLeafValue | undefined
    }
  | {
      readonly kind: "object"
      readonly status: StorageDiffStatus
      readonly objectKind: StorageObjectKind
      readonly typeName?: string
      readonly entries: readonly StorageDiffEntry[]
    }

const scalar = (value: string, rawValue?: string, typeName?: string): StorageLeafValue => ({
  kind: "scalar",
  value,
  rawValue,
  typeName,
})

const nullValue = (): StorageLeafValue => ({
  kind: "null",
})

const voidValue = (): StorageLeafValue => ({
  kind: "void",
})

const addressValue = (value: string): StorageLeafValue => ({
  kind: "address",
  value,
})

const booleanValue = (value: boolean): StorageLeafValue => ({
  kind: "boolean",
  value,
})

const objectValue = (
  entries: readonly StorageValueEntry[],
  typeName?: string,
  objectKind: StorageObjectKind = "object",
): Extract<StorageValue, {readonly kind: "object"}> => ({
  kind: "object",
  objectKind,
  typeName,
  entries,
})

const stringifyParsedValue = (value: ParsedValue): string => {
  switch (value.kind) {
    case "null": {
      return "null"
    }
    case "void": {
      return "void"
    }
    case "address":
    case "scalar": {
      return value.value
    }
    case "boolean": {
      return value.value ? "true" : "false"
    }
    case "array": {
      return `[${value.items.map(item => stringifyParsedValue(item)).join(", ")}]`
    }
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

const normalizeParsedValue = (value: ParsedValue): StorageValue => {
  switch (value.kind) {
    case "null": {
      return nullValue()
    }
    case "void": {
      return voidValue()
    }
    case "boolean": {
      return booleanValue(value.value)
    }
    case "scalar": {
      return scalar(value.value, value.rawValue, value.typeName)
    }
    case "address": {
      return addressValue(value.value)
    }
    case "array": {
      return objectValue(
        value.items.map((item, index) => ({
          key: `[${index}]`,
          value: normalizeParsedValue(item),
        })),
        "array",
        "array",
      )
    }
    case "map": {
      return objectValue(
        value.entries.map(entry => ({
          key: stringifyParsedValue(entry.key),
          value: normalizeParsedValue(entry.value),
        })),
        value.typeName ?? "map",
        "map",
      )
    }
    case "object": {
      return objectValue(
        value.entries.map(entry => ({
          key: entry.key,
          value: normalizeParsedValue(entry.value),
        })),
        value.typeName,
        "object",
      )
    }
  }
}

const normalizeStorage = (value: ParsedContractStorage | undefined): StorageValue | undefined => {
  if (!value) {
    return undefined
  }

  const normalized = normalizeParsedValue(value.value)
  if (normalized.kind === "object") {
    return normalized.typeName
      ? normalized
      : objectValue(normalized.entries, value.name, normalized.objectKind)
  }

  return objectValue([{key: "value", value: normalized}], value.name)
}

const toAddedDiff = (value: StorageValue): StorageDiffNode => {
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

const toRemovedDiff = (value: StorageValue): StorageDiffNode => {
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

const areLeafValuesEqual = (before: StorageLeafValue, after: StorageLeafValue): boolean => {
  if (before.kind !== after.kind) {
    return false
  }

  if (before.kind === "null" && after.kind === "null") {
    return true
  }

  if (before.kind === "boolean" && after.kind === "boolean") {
    return before.value === after.value
  }

  if (before.kind === "address" && after.kind === "address") {
    return before.value === after.value
  }

  if (before.kind === "scalar" && after.kind === "scalar") {
    return (before.rawValue ?? before.value) === (after.rawValue ?? after.value)
  }

  return false
}

const diffStorageValues = (
  before: StorageValue | undefined,
  after: StorageValue | undefined,
): StorageDiffNode | undefined => {
  if (!before && !after) {
    return undefined
  }

  if (!before) {
    return after ? toAddedDiff(after) : undefined
  }

  if (!after) {
    return toRemovedDiff(before)
  }

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

  const orderedKeys: string[] = [
    ...before.entries.map(entry => entry.key),
    ...after.entries
      .map(entry => entry.key)
      .filter(key => !before.entries.some(beforeEntry => beforeEntry.key === key)),
  ]

  const beforeEntryMap = new Map<string, StorageValue>(
    before.entries.map(entry => [entry.key, entry.value]),
  )
  const afterEntryMap = new Map<string, StorageValue>(
    after.entries.map(entry => [entry.key, entry.value]),
  )

  const entries = orderedKeys.flatMap(key => {
    const value = diffStorageValues(beforeEntryMap.get(key), afterEntryMap.get(key))
    return value ? [{key, value}] : []
  })

  const status: StorageDiffStatus =
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

export const buildStorageDiff = (
  before: ParsedContractStorage | undefined,
  after: ParsedContractStorage | undefined,
): StorageDiffNode | undefined => {
  return diffStorageValues(normalizeStorage(before), normalizeStorage(after))
}

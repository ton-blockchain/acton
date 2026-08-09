import {Address, Cell, Dictionary, ExternalAddress} from "@ton/core"
import type {SymTable} from "@ton/tolk-abi-to-typescript"

import {
  formatTolkDocComment,
  formatTolkIdentifier,
  formatType,
  tryAliasTargetTyIdx,
  tryTyByIdx,
} from "./abiFormatting"

interface CellLike {
  readonly asCell: () => Cell
}

interface StructFieldInfo {
  readonly tyIdx: number
  readonly description?: string
}

export type FormattedAbiDecodedValue =
  | {readonly kind: "plain"; readonly value: string}
  | {readonly kind: "tolk"; readonly value: string}

export function formatAbiDecodedValue(
  decoded: unknown,
  symbols: SymTable,
  returnTyIdx: number,
): FormattedAbiDecodedValue {
  const displayValue = decodedDisplayValue(decoded)
  if (isPlainDecodedValue(displayValue)) {
    return {kind: "plain", value: String(displayValue)}
  }
  return {
    kind: "tolk",
    value: formatDecodedTolkNode(displayValue, symbols, returnTyIdx, 0),
  }
}

function decodedDisplayValue(value: unknown): unknown {
  if (typeof value === "bigint") return value
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
    return value
  }
  if (value === null || value === undefined) return value
  if (value instanceof Address) return value.toString()
  if (value instanceof ExternalAddress) return value.toString()
  if (value instanceof Cell) return formatCellHex(value)
  if (value instanceof Dictionary) {
    return Object.fromEntries(
      [...value].map(([key, item]) => [
        String(decodedDisplayValue(key)),
        decodedDisplayValue(item),
      ]),
    )
  }
  if (value instanceof Map) {
    return Object.fromEntries(
      [...value].map(([key, item]) => [
        String(decodedDisplayValue(key)),
        decodedDisplayValue(item),
      ]),
    )
  }
  if (Array.isArray(value)) return value.map(item => decodedDisplayValue(item))
  if (isCellLike(value)) return formatCellHex(value.asCell())
  if (isRecord(value)) {
    return Object.fromEntries(
      Object.entries(value).map(([key, item]) => [key, decodedDisplayValue(item)]),
    )
  }
  return stringifyUnknown(value)
}

function formatDecodedTolkNode(
  value: unknown,
  symbols: SymTable,
  tyIdx: number | undefined,
  indent: number,
): string {
  const pad = " ".repeat(indent * 4)
  const nextPad = " ".repeat((indent + 1) * 4)

  if (Array.isArray(value)) {
    if (value.length === 0) return "[]"
    const itemTyIdx = tyIdx === undefined ? undefined : getCollectionItemTyIdx(symbols, tyIdx)
    return `[\n${value
      .map(item => `${nextPad}${formatDecodedTolkNode(item, symbols, itemTyIdx, indent + 1)}`)
      .join("\n")}\n${pad}]`
  }

  if (isRecord(value)) {
    const entries = Object.entries(value).filter(([key]) => key !== "$")
    const typeName =
      typeof value.$ === "string"
        ? value.$
        : tyIdx === undefined
          ? undefined
          : sanitizeTolkTypeName(formatType(symbols, tyIdx))
    const fields =
      tyIdx === undefined ? new Map<string, StructFieldInfo>() : getStructFields(symbols, tyIdx)

    if (entries.length === 0) return typeName ? `${typeName} {}` : "{}"

    const body = entries
      .map(([key, item]) => {
        const field = fields.get(key)
        const comment = field?.description
          ? `${formatTolkDocComment(field.description, nextPad.length)}\n`
          : ""
        return `${comment}${nextPad}${formatTolkIdentifier(key)}: ${formatDecodedTolkNode(
          item,
          symbols,
          field?.tyIdx,
          indent + 1,
        )}`
      })
      .join("\n")
    return `${typeName ? `${typeName} ` : ""}{\n${body}\n${pad}}`
  }

  return formatDecodedTolkScalar(value)
}

function getStructFields(symbols: SymTable, tyIdx: number): Map<string, StructFieldInfo> {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) return new Map()

  switch (ty.kind) {
    case "StructRef": {
      let fields: readonly {
        readonly name: string
        readonly ty_idx: number
        readonly client_ty_idx?: number
        readonly description?: string
      }[] = []
      try {
        fields = symbols.getStruct(ty.struct_name).fields
      } catch {
        return new Map()
      }
      return new Map(
        fields.map(field => [
          field.name,
          {
            tyIdx: field.client_ty_idx ?? field.ty_idx,
            description: field.description,
          },
        ]),
      )
    }
    case "AliasRef": {
      const targetTyIdx = tryAliasTargetTyIdx(symbols, tyIdx)
      return targetTyIdx === undefined ? new Map() : getStructFields(symbols, targetTyIdx)
    }
    case "nullable":
      return getStructFields(symbols, ty.inner_ty_idx)
    default:
      return new Map()
  }
}

function getCollectionItemTyIdx(symbols: SymTable, tyIdx: number): number | undefined {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) return undefined

  switch (ty.kind) {
    case "arrayOf":
    case "lispListOf":
    case "cellOf":
      return ty.inner_ty_idx
    case "nullable":
      return getCollectionItemTyIdx(symbols, ty.inner_ty_idx)
    case "AliasRef": {
      const targetTyIdx = tryAliasTargetTyIdx(symbols, tyIdx)
      return targetTyIdx === undefined ? undefined : getCollectionItemTyIdx(symbols, targetTyIdx)
    }
    default:
      return undefined
  }
}

function formatDecodedTolkScalar(value: unknown): string {
  if (value === null || value === undefined) return "null"
  if (typeof value === "number" || typeof value === "boolean" || typeof value === "bigint") {
    return String(value)
  }
  if (typeof value === "string") return JSON.stringify(value)
  return JSON.stringify(stringifyUnknown(value))
}

function formatCellHex(cell: Cell): string {
  return cell.toBoc().toString("hex")
}

function isPlainDecodedValue(value: unknown): boolean {
  return (
    value === null ||
    value === undefined ||
    typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean" ||
    typeof value === "bigint"
  )
}

function isCellLike(value: unknown): value is CellLike {
  return isRecord(value) && typeof value.asCell === "function"
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value)
}

function sanitizeTolkTypeName(value: string): string {
  return value.replace(/\?.*$/, "").trim()
}

function stringifyUnknown(value: unknown): string {
  if (typeof value === "string") return value
  if (typeof value === "number" || typeof value === "boolean" || typeof value === "bigint") {
    return String(value)
  }
  if (value instanceof Error) return value.message
  const json = JSON.stringify(value)
  return json ?? Object.prototype.toString.call(value)
}

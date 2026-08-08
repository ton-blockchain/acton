import {Address, BitString, Builder, Cell, Dictionary, Slice} from "@ton/core"
import type {ContractABI, SymTable, Ty} from "@ton/tolk-abi-to-typescript"
import {renderTy} from "@ton/tolk-abi-to-typescript"
import {shortenMiddle} from "@acton/ui"

import type {ParsedValue} from "../model/transaction"

const HEX_PREVIEW_HEAD_LENGTH = 24
const HEX_PREVIEW_TAIL_LENGTH = 8

export interface ParsedValueTypeContext {
  readonly symbols: SymTable
  readonly tyIdx: number
  readonly abi?: ContractABI
  readonly abiCandidates?: readonly ContractABI[]
  readonly nestedPayloadDepth?: number
  readonly decodeRemaining?: (
    slice: Slice,
    context: ParsedValueTypeContext,
  ) => ParsedValue | undefined
}

const isCellWrapperObject = (value: Record<string, unknown>): value is {ref: unknown} => {
  const keys = Object.keys(value)
  return (
    (keys.length === 1 && keys[0] === "ref") ||
    (value.$ === "Cell" && keys.length === 2 && keys.includes("$") && keys.includes("ref"))
  )
}

const formatHexPreview = (hex: string): string => {
  return shortenMiddle(hex, {start: HEX_PREVIEW_HEAD_LENGTH, end: HEX_PREVIEW_TAIL_LENGTH})
}

const formatSerializedCellPreview = (
  typeName: "Cell" | "Slice" | "Builder",
  cell: Cell,
): string => {
  if (cell.bits.length === 0 && cell.refs.length === 0) {
    return `<empty ${typeName.toLowerCase()}>`
  }

  const hex = cell.toBoc({idx: false, crc32: false}).toString("hex")
  return `${typeName}(${formatHexPreview(hex)})`
}

const toSerializedCellScalar = (
  typeName: "Cell" | "Slice" | "Builder",
  cell: Cell,
): ParsedValue => ({
  kind: "scalar",
  value: formatSerializedCellPreview(typeName, cell),
  rawValue: cell.toBoc({idx: false, crc32: false}).toString("hex"),
  typeName,
})

const withTypeIndex = (context: ParsedValueTypeContext, tyIdx: number): ParsedValueTypeContext => ({
  ...context,
  tyIdx,
})

const renderTypeName = (context: ParsedValueTypeContext | undefined): string | undefined => {
  if (!context) {
    return undefined
  }

  try {
    return renderTy(context.symbols, context.tyIdx)
  } catch {
    return undefined
  }
}

const tryGetTy = (symbols: SymTable, tyIdx: number): Ty | undefined => {
  try {
    return symbols.tyByIdx(tyIdx)
  } catch {
    return undefined
  }
}

const sliceFromRemainingValue = (value: unknown): Slice | undefined => {
  if (value instanceof Slice) {
    return value
  }

  if (value instanceof Cell) {
    return value.beginParse()
  }

  return undefined
}

const valueToBitString = (value: unknown, length: number): BitString | undefined => {
  if (BitString.isBitString(value)) {
    return value.length === length ? value : undefined
  }

  if (!(value instanceof Slice) || value.remainingRefs !== 0 || value.remainingBits !== length) {
    return undefined
  }

  return value.clone().loadBits(length)
}

const toParsedValueWithType = (
  value: unknown,
  context: ParsedValueTypeContext,
): ParsedValue | undefined => {
  const ty = tryGetTy(context.symbols, context.tyIdx)
  if (!ty) {
    return undefined
  }

  switch (ty.kind) {
    case "remaining": {
      const remainingSlice = sliceFromRemainingValue(value)
      return remainingSlice ? context.decodeRemaining?.(remainingSlice, context) : undefined
    }
    case "bitsN": {
      const bitString = valueToBitString(value, ty.n)
      if (!bitString) {
        return undefined
      }

      return {
        kind: "scalar",
        value: bitString.toString(),
        typeName: renderTy(context.symbols, context.tyIdx),
      }
    }
    case "nullable": {
      return value === null
        ? {kind: "null"}
        : toParsedValue(value, withTypeIndex(context, ty.inner_ty_idx))
    }
    case "cellOf": {
      if (typeof value !== "object" || value === null || !("ref" in value)) {
        return undefined
      }

      return toParsedValue(
        (value as {readonly ref: unknown}).ref,
        withTypeIndex(context, ty.inner_ty_idx),
      )
    }
    case "arrayOf":
    case "lispListOf": {
      if (!Array.isArray(value)) {
        return undefined
      }

      return {
        kind: "array",
        items: value.map(item => toParsedValue(item, withTypeIndex(context, ty.inner_ty_idx))),
      }
    }
    case "tensor":
    case "shapedTuple": {
      if (!Array.isArray(value)) {
        return undefined
      }

      return {
        kind: "array",
        items: value.map((item, index) =>
          toParsedValue(item, withTypeIndex(context, ty.items_ty_idx[index] ?? context.tyIdx)),
        ),
      }
    }
    case "mapKV": {
      if (!(value instanceof Dictionary)) {
        return undefined
      }

      return {
        kind: "map",
        typeName: renderTy(context.symbols, context.tyIdx),
        entries: [...value].map(([key, itemValue]) => ({
          key: toParsedValue(key, withTypeIndex(context, ty.key_ty_idx)),
          value: toParsedValue(itemValue, withTypeIndex(context, ty.value_ty_idx)),
        })),
      }
    }
    case "EnumRef": {
      if (typeof value !== "bigint") {
        return undefined
      }

      const enumRef = context.symbols.getEnum(ty.enum_name)
      const member = enumRef.members.find(candidate => BigInt(candidate.value) === value)
      return {
        kind: "scalar",
        value: member ? `${ty.enum_name}.${member.name} (${value})` : `${ty.enum_name}(${value})`,
        rawValue: value.toString(),
        typeName: ty.enum_name,
      }
    }
    case "StructRef": {
      const structRef = context.symbols.getStruct(ty.struct_name)
      if (structRef.custom_pack_unpack?.unpack_from_slice) {
        return undefined
      }

      if (typeof value !== "object" || value === null) {
        return undefined
      }

      const objectValue = value as Record<string, unknown>
      return {
        kind: "object",
        typeName: renderTy(context.symbols, context.tyIdx),
        entries: context.symbols.structFieldsOf(context.tyIdx, false).map(field => ({
          key: field.name,
          value: toParsedValue(objectValue[field.name], withTypeIndex(context, field.ty_idx)),
        })),
      }
    }
    case "AliasRef": {
      const aliasRef = context.symbols.getAlias(ty.alias_name)
      if (aliasRef.custom_pack_unpack?.unpack_from_slice) {
        return undefined
      }

      const target = context.symbols.aliasTargetOf(context.tyIdx)
      return toParsedValue(value, withTypeIndex(context, target.ty_idx))
    }
    default: {
      return undefined
    }
  }
}

export const toParsedValue = (
  value: unknown,
  typeContext?: ParsedValueTypeContext,
): ParsedValue => {
  let typedValue: ParsedValue | undefined
  if (typeContext) {
    try {
      typedValue = toParsedValueWithType(value, typeContext)
    } catch {
      typedValue = undefined
    }
  }

  if (typedValue) {
    return typedValue
  }

  if (value === null) {
    return {kind: "null"}
  }

  if (value === undefined) {
    return {kind: "void"}
  }

  if (typeof value === "boolean") {
    return {kind: "boolean", value}
  }

  if (typeof value === "bigint" || typeof value === "number" || typeof value === "string") {
    return {kind: "scalar", value: value.toString(), typeName: renderTypeName(typeContext)}
  }

  if (value instanceof Address) {
    return {kind: "address", value: value.toString()}
  }

  if (value instanceof Cell) {
    return toSerializedCellScalar("Cell", value)
  }

  if (value instanceof Slice) {
    return toSerializedCellScalar("Slice", value.asCell())
  }

  if (BitString.isBitString(value)) {
    return {kind: "scalar", value: value.toString()}
  }

  if (value instanceof Builder) {
    return toSerializedCellScalar("Builder", value.asCell())
  }

  if (value instanceof Dictionary) {
    return {
      kind: "map",
      entries: [...value].map(([key, itemValue]) => ({
        key: toParsedValue(key),
        value: toParsedValue(itemValue),
      })),
    }
  }

  if (Array.isArray(value)) {
    return {
      kind: "array",
      items: value.map(item => toParsedValue(item)),
    }
  }

  if (typeof value === "object") {
    const objectValue = value as Record<string, unknown>
    if (isCellWrapperObject(objectValue)) {
      return toParsedValue(objectValue.ref)
    }

    const typeName = typeof objectValue.$ === "string" ? objectValue.$ : undefined
    if (
      typeName === "void" &&
      Object.hasOwn(objectValue, "value") &&
      objectValue.value === undefined
    ) {
      return {kind: "void"}
    }

    return {
      kind: "object",
      typeName,
      entries: Object.entries(objectValue)
        .filter(([key]) => key !== "$")
        .map(([key, itemValue]) => ({
          key,
          value: toParsedValue(itemValue),
        })),
    }
  }

  return {kind: "scalar", value: Object.prototype.toString.call(value)}
}

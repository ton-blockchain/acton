import {Buffer} from "node:buffer"

import {Address, Cell} from "@ton/core"
import {
  renderTy,
  type DynamicCtx,
  type SymTable,
  type UnionVariant,
} from "@ton/tolk-abi-to-typescript"

export const SAMPLE_ADDRESS = "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c"

export function normalizeSimpleAbiDynamicArg(
  ctx: DynamicCtx,
  tyIdx: number,
  value: string,
): unknown {
  const ty = ctx.symbols.tyByIdx(tyIdx)
  switch (ty.kind) {
    case "int":
    case "intN":
    case "uintN":
    case "varintN":
    case "varuintN":
    case "coins":
    case "EnumRef": {
      return BigInt(requireArgValue(value, "Number argument"))
    }
    case "bool": {
      if (value === "true") return true
      if (value === "false") return false
      throw new Error("Boolean argument must be true or false.")
    }
    case "string": {
      return value
    }
    case "address":
    case "addressExt": {
      return Address.parse(requireArgValue(value, "Address argument"))
    }
    case "addressOpt": {
      return value.trim() ? Address.parse(value.trim()) : null
    }
    case "addressAny": {
      return value.trim() ? Address.parse(value.trim()) : "none"
    }
    case "cell": {
      return parseAbiCellArg(value)
    }
    case "builder": {
      return parseAbiCellArg(value).asBuilder()
    }
    case "slice":
    case "remaining":
    case "bitsN": {
      return parseAbiCellArg(value).beginParse()
    }
    case "nullable": {
      return value.trim() ? normalizeSimpleAbiDynamicArg(ctx, ty.inner_ty_idx, value) : null
    }
    case "AliasRef": {
      const target = ctx.symbols.aliasTargetOf(tyIdx)
      return normalizeSimpleAbiDynamicArg(ctx, target.ty_idx, value)
    }
    default: {
      return normalizeAbiDynamicArg(ctx, tyIdx, value)
    }
  }
}

export function normalizeAbiDynamicArg(ctx: DynamicCtx, tyIdx: number, value: unknown): unknown {
  const ty = ctx.symbols.tyByIdx(tyIdx)
  switch (ty.kind) {
    case "int":
    case "intN":
    case "uintN":
    case "varintN":
    case "varuintN":
    case "coins":
    case "EnumRef": {
      return typeof value === "string" ? BigInt(value) : value
    }
    case "address":
    case "addressExt": {
      return typeof value === "string" ? Address.parse(value) : value
    }
    case "addressOpt": {
      if (value === null || value === undefined) return null
      if (typeof value === "string") {
        const trimmed = value.trim()
        return trimmed ? Address.parse(trimmed) : null
      }
      return value
    }
    case "addressAny": {
      return typeof value === "string" && value !== "none" ? Address.parse(value) : value
    }
    case "cell": {
      return typeof value === "string" ? parseAbiCellArg(value) : value
    }
    case "builder": {
      return typeof value === "string" ? parseAbiCellArg(value).asBuilder() : value
    }
    case "slice":
    case "remaining":
    case "bitsN": {
      return typeof value === "string" ? parseAbiCellArg(value).beginParse() : value
    }
    case "cellOf": {
      if (isRecord(value) && "ref" in value) {
        return {ref: normalizeAbiDynamicArg(ctx, ty.inner_ty_idx, value.ref)}
      }
      return value
    }
    case "nullable": {
      // biome-ignore lint/suspicious/noDoubleEquals: ABI nullish values intentionally map to TVM null.
      return value == undefined ? null : normalizeAbiDynamicArg(ctx, ty.inner_ty_idx, value)
    }
    case "arrayOf":
    case "lispListOf": {
      return Array.isArray(value)
        ? value.map(item => normalizeAbiDynamicArg(ctx, ty.inner_ty_idx, item))
        : value
    }
    case "tensor":
    case "shapedTuple": {
      return Array.isArray(value)
        ? value.map((item, index) => normalizeAbiDynamicArg(ctx, ty.items_ty_idx[index], item))
        : value
    }
    case "mapKV": {
      if (isRecord(value)) {
        return new Map(
          Object.entries(value).map(([key, item]) => [
            normalizeDynamicMapKey(ctx, ty.key_ty_idx, key),
            normalizeAbiDynamicArg(ctx, ty.value_ty_idx, item),
          ]),
        )
      }
      return value
    }
    case "StructRef": {
      if (isRecord(value)) {
        return Object.fromEntries(
          ctx.symbols
            .structFieldsOf(tyIdx, false)
            .map(field => [field.name, normalizeAbiDynamicArg(ctx, field.ty_idx, value[field.name])]),
        )
      }
      return value
    }
    case "AliasRef": {
      const target = ctx.symbols.aliasTargetOf(tyIdx)
      return normalizeAbiDynamicArg(ctx, target.ty_idx, value)
    }
    default: {
      return value
    }
  }
}

export function parseAbiCellArg(value: string): Cell {
  const trimmed = requireArgValue(value, "Cell argument")
  const hex = trimmed.startsWith("0x") ? trimmed.slice(2) : trimmed
  if (/^(?:[0-9a-fA-F]{2})+$/.test(hex)) {
    return Cell.fromBoc(Buffer.from(hex, "hex"))[0]
  }
  return Cell.fromBase64(trimmed)
}

export function sampleAbiValueForTy(
  symbols: SymTable,
  tyIdx: number,
  visited = new Set<number>(),
): unknown {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) return undefined
  switch (ty.kind) {
    case "int":
    case "intN":
    case "uintN":
    case "varintN":
    case "varuintN":
    case "coins":
    case "EnumRef": {
      return "0"
    }
    case "bool": {
      return false
    }
    case "string": {
      return ""
    }
    case "address":
    case "addressExt":
    case "addressAny": {
      return SAMPLE_ADDRESS
    }
    case "addressOpt": {
      return null
    }
    case "cell":
    case "slice":
    case "builder":
    case "bitsN":
    case "remaining": {
      return "te6ccgEBAQEAAgAAAA=="
    }
    case "nullable": {
      return null
    }
    case "cellOf": {
      return {ref: sampleAbiValueForTy(symbols, ty.inner_ty_idx, visited)}
    }
    case "arrayOf":
    case "lispListOf": {
      return []
    }
    case "tensor":
    case "shapedTuple": {
      return ty.items_ty_idx.map(itemTyIdx => sampleAbiValueForTy(symbols, itemTyIdx, visited))
    }
    case "mapKV": {
      return {}
    }
    case "StructRef": {
      if (visited.has(tyIdx)) return {}
      visited.add(tyIdx)
      return Object.fromEntries(
        symbols
          .structFieldsOf(tyIdx, false)
          .map(field => [field.name, sampleAbiValueForTy(symbols, field.ty_idx, visited)]),
      )
    }
    case "AliasRef": {
      const targetTyIdx = tryAliasTargetTyIdx(symbols, tyIdx)
      return targetTyIdx === undefined
        ? undefined
        : sampleAbiValueForTy(symbols, targetTyIdx, visited)
    }
    case "union": {
      const variant = createSampleUnionLabels(symbols, ty.variants)[0]
      if (!variant) return undefined
      if (tryTyByIdx(symbols, variant.variant_ty_idx)?.kind === "nullLiteral") {
        return null
      }

      const sample = sampleAbiValueForTy(symbols, variant.variant_ty_idx, visited)
      if (variant.hasValueField) {
        return {$: variant.labelStr, value: sample}
      }
      if (isRecord(sample)) {
        return {$: variant.labelStr, ...sample}
      }
      return {$: variant.labelStr}
    }
    case "nullLiteral": {
      return null
    }
    default: {
      return undefined
    }
  }
}

export function stringifyAbiJson(value: unknown): string {
  return JSON.stringify(
    value,
    (_key, item) => {
      if (typeof item === "bigint") {
        return item.toString()
      }
      if (Address.isAddress(item)) {
        return item.toString()
      }
      if (item instanceof Cell) {
        return item.toBoc().toString("base64")
      }
      return item
    },
    2,
  )
}

function normalizeDynamicMapKey(ctx: DynamicCtx, tyIdx: number, value: string): unknown {
  const ty = ctx.symbols.tyByIdx(tyIdx)
  switch (ty.kind) {
    case "int":
    case "intN":
    case "uintN":
    case "varintN":
    case "varuintN":
    case "coins":
    case "EnumRef": {
      return BigInt(value)
    }
    case "address":
    case "addressOpt":
    case "addressAny": {
      return Address.parse(value)
    }
    case "AliasRef": {
      const target = ctx.symbols.aliasTargetOf(tyIdx)
      return normalizeDynamicMapKey(ctx, target.ty_idx, value)
    }
    default: {
      return value
    }
  }
}

function requireArgValue(value: string, label: string): string {
  const trimmed = value.trim()
  if (!trimmed) {
    throw new Error(`${label} is required.`)
  }
  return trimmed
}

function tryTyByIdx(symbols: SymTable, tyIdx: number) {
  try {
    return symbols.tyByIdx(tyIdx)
  } catch {
    return undefined
  }
}

function tryAliasTargetTyIdx(symbols: SymTable, tyIdx: number): number | undefined {
  try {
    return symbols.aliasTargetOf(tyIdx).ty_idx
  } catch {
    return undefined
  }
}

function createSampleUnionLabels(
  symbols: SymTable,
  variants: readonly UnionVariant[],
): readonly (UnionVariant & {readonly labelStr: string; readonly hasValueField: boolean})[] {
  const labels = variants.map(variant => createSampleTypeLabel(symbols, variant.variant_ty_idx))
  const duplicatedLabels = new Set(labels.filter((label, index) => labels.indexOf(label) !== index))

  return variants.map((variant, index) => {
    const labelTy = tryTyByIdx(symbols, variant.variant_ty_idx)
    if (labelTy?.kind === "nullLiteral") {
      return {...variant, labelStr: "", hasValueField: false}
    }

    return {
      ...variant,
      labelStr: duplicatedLabels.has(labels[index])
        ? safeRenderTy(symbols, variant.variant_ty_idx)
        : labels[index],
      hasValueField: duplicatedLabels.has(labels[index])
        ? true
        : !isStructWithItsOwnLabel(symbols, variant.variant_ty_idx),
    }
  })
}

function createSampleTypeLabel(symbols: SymTable, tyIdx: number): string {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) {
    return `ty#${tyIdx}`
  }

  switch (ty.kind) {
    case "nullable": {
      return `${createSampleTypeLabel(symbols, ty.inner_ty_idx)}?`
    }
    case "cellOf": {
      return "Cell"
    }
    case "arrayOf": {
      return "array"
    }
    case "lispListOf": {
      return "lisp_list"
    }
    case "tensor": {
      return "tensor"
    }
    case "shapedTuple": {
      return "shaped"
    }
    case "mapKV": {
      return "map"
    }
    case "StructRef": {
      return ty.struct_name
    }
    case "AliasRef": {
      const targetTyIdx = tryAliasTargetTyIdx(symbols, tyIdx)
      return targetTyIdx === undefined ? ty.alias_name : createSampleTypeLabel(symbols, targetTyIdx)
    }
    case "union": {
      return ty.variants.map(variant => createSampleTypeLabel(symbols, variant.variant_ty_idx)).join("|")
    }
    case "nullLiteral": {
      return ""
    }
    default: {
      return safeRenderTy(symbols, tyIdx)
    }
  }
}

function isStructWithItsOwnLabel(symbols: SymTable, tyIdx: number): boolean {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (ty?.kind === "StructRef") return true
  if (ty?.kind === "AliasRef") {
    const targetTyIdx = tryAliasTargetTyIdx(symbols, tyIdx)
    return targetTyIdx !== undefined && isStructWithItsOwnLabel(symbols, targetTyIdx)
  }
  return false
}

function safeRenderTy(symbols: SymTable, tyIdx: number): string {
  try {
    return renderTy(symbols, tyIdx)
  } catch {
    return `ty#${tyIdx}`
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value)
}

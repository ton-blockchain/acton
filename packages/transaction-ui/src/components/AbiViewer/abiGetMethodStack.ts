import {Cell, TupleReader, type ContractProvider, type TupleItem} from "@ton/core"
import type {SymTable} from "@ton/tolk-abi-to-typescript"

export interface AbiGetMethodStackEntry {
  readonly type: string
  readonly value: unknown
}

export interface AbiGetMethodResponse {
  readonly gas_used: number | string
  readonly exit_code: number
  readonly stack: readonly AbiGetMethodStackEntry[]
  readonly vm_log?: string
}

export type AbiRunGetMethod = (
  method: string | number,
  stack: readonly AbiGetMethodStackEntry[],
) => Promise<AbiGetMethodResponse>

interface AbiGetMethodResultSchema {
  readonly symbols: SymTable
  readonly returnTyIdx: number
}

export function createAbiGetMethodProvider(
  runGetMethod: AbiRunGetMethod,
  onResult: (result: AbiGetMethodResponse) => void,
  resultSchema?: AbiGetMethodResultSchema,
): ContractProvider {
  return {
    async get(name, args) {
      const result = await runGetMethod(
        name,
        args.map(value => tupleItemToStackEntry(value)),
      )
      onResult(result)
      if (result.exit_code !== 0) {
        throw new Error(`Get method exited with code ${result.exit_code}.`)
      }
      const stack = result.stack.map(value => stackEntryToTupleItem(value))
      if (resultSchema) {
        normalizeTupleItemsForAbi(stack, resultSchema.symbols, resultSchema.returnTyIdx)
      }
      return {
        stack: new TupleReader(stack),
        gasUsed: BigInt(result.gas_used),
        logs: result.vm_log ?? "",
      }
    },
  } as ContractProvider
}

function normalizeTupleItemsForAbi(
  items: TupleItem[],
  symbols: SymTable,
  tyIdx: number,
  offset = 0,
): number {
  const ty = symbols.tyByIdx(tyIdx)

  switch (ty.kind) {
    case "void":
      return 0
    case "tensor": {
      let consumed = 0
      for (const itemTyIdx of ty.items_ty_idx) {
        consumed += normalizeTupleItemsForAbi(items, symbols, itemTyIdx, offset + consumed)
      }
      return consumed
    }
    case "StructRef": {
      let consumed = 0
      for (const field of symbols.structFieldsOf(tyIdx, true)) {
        consumed += normalizeTupleItemsForAbi(items, symbols, field.ty_idx, offset + consumed)
      }
      return consumed
    }
    case "AliasRef":
      return normalizeTupleItemsForAbi(items, symbols, symbols.aliasTargetOf(tyIdx).ty_idx, offset)
    case "nullable": {
      const width = ty.stack_width ?? 1
      if (width === 1 && items[offset]?.type === "null") return 1
      if (ty.stack_type_id) {
        const typeId = items[offset + width - 1]
        if (typeId?.type === "int" && typeId.value !== 0n) {
          normalizeTupleItemsForAbi(items, symbols, ty.inner_ty_idx, offset)
        }
        return width
      }
      return normalizeTupleItemsForAbi(items, symbols, ty.inner_ty_idx, offset)
    }
    case "union": {
      const width = ty.stack_width ?? 1
      const typeId = items[offset + width - 1]
      const variant =
        typeId?.type === "int"
          ? ty.variants.find(candidate => candidate.stack_type_id === Number(typeId.value))
          : undefined
      if (variant) {
        const variantWidth = variant.stack_width ?? abiStackWidth(symbols, variant.variant_ty_idx)
        const variantOffset = offset + width - 1 - variantWidth
        normalizeTupleItemsForAbi(items, symbols, variant.variant_ty_idx, variantOffset)
      }
      return width
    }
    case "arrayOf": {
      const tuple = items[offset]
      if (tuple?.type === "tuple") {
        normalizeRepeatedTupleItems(tuple.items, symbols, ty.inner_ty_idx)
      }
      return 1
    }
    case "lispListOf": {
      normalizeLispListItem(items[offset], symbols, ty.inner_ty_idx)
      return 1
    }
    case "shapedTuple": {
      const tuple = items[offset]
      if (tuple?.type === "tuple") {
        let nestedOffset = 0
        for (const itemTyIdx of ty.items_ty_idx) {
          nestedOffset += normalizePossiblyNestedItem(tuple.items, symbols, itemTyIdx, nestedOffset)
        }
      }
      return 1
    }
    case "cell":
    case "cellOf":
    case "mapKV":
    case "string":
      normalizeCellLikeItem(items, offset, "cell")
      return 1
    case "builder":
      normalizeCellLikeItem(items, offset, "builder")
      return 1
    case "slice":
    case "remaining":
    case "address":
    case "addressOpt":
    case "addressExt":
    case "addressAny":
    case "bitsN":
      normalizeCellLikeItem(items, offset, "slice")
      return 1
    default:
      return 1
  }
}

function normalizeRepeatedTupleItems(items: TupleItem[], symbols: SymTable, tyIdx: number): void {
  for (let offset = 0; offset < items.length; ) {
    offset += normalizePossiblyNestedItem(items, symbols, tyIdx, offset)
  }
}

function normalizePossiblyNestedItem(
  items: TupleItem[],
  symbols: SymTable,
  tyIdx: number,
  offset: number,
): number {
  if (abiStackWidth(symbols, tyIdx) === 1) {
    return normalizeTupleItemsForAbi(items, symbols, tyIdx, offset)
  }
  const tuple = items[offset]
  if (tuple?.type === "tuple") normalizeTupleItemsForAbi(tuple.items, symbols, tyIdx)
  return 1
}

function normalizeLispListItem(
  item: TupleItem | undefined,
  symbols: SymTable,
  tyIdx: number,
): void {
  if (!item || item.type === "null") return
  if (item.type !== "tuple" || item.items.length !== 2) return

  normalizePossiblyNestedItem(item.items, symbols, tyIdx, 0)
  normalizeLispListItem(item.items[1], symbols, tyIdx)
}

function normalizeCellLikeItem(
  items: TupleItem[],
  offset: number,
  expectedType: "builder" | "cell" | "slice",
): void {
  const item = items[offset]
  if (item?.type !== "builder" && item?.type !== "cell" && item?.type !== "slice") return
  items[offset] = {type: expectedType, cell: item.cell}
}

function abiStackWidth(symbols: SymTable, tyIdx: number): number {
  const ty = symbols.tyByIdx(tyIdx)
  switch (ty.kind) {
    case "void":
      return 0
    case "tensor":
      return ty.items_ty_idx.reduce(
        (width, itemTyIdx) => width + abiStackWidth(symbols, itemTyIdx),
        0,
      )
    case "StructRef":
      return symbols
        .structFieldsOf(tyIdx, true)
        .reduce((width, field) => width + abiStackWidth(symbols, field.ty_idx), 0)
    case "AliasRef":
      return abiStackWidth(symbols, symbols.aliasTargetOf(tyIdx).ty_idx)
    case "nullable":
      return ty.stack_width ?? 1
    case "union":
      return ty.stack_width ?? 1
    default:
      return 1
  }
}

function tupleItemToStackEntry(item: TupleItem): AbiGetMethodStackEntry {
  switch (item.type) {
    case "int":
      return {type: "num", value: item.value.toString()}
    case "null":
      return {type: "null", value: null}
    case "cell":
    case "slice":
    case "builder":
      return {type: item.type, value: item.cell.toBoc().toString("base64")}
    case "tuple":
      return {type: "tuple", value: item.items.map(value => tupleItemToStackEntry(value))}
    case "nan":
      throw new Error("NaN tuple items cannot be passed to runGetMethod.")
  }
}

function stackEntryToTupleItem(entry: AbiGetMethodStackEntry): TupleItem {
  switch (entry.type) {
    case "num":
      return entry.value === "NaN"
        ? {type: "nan"}
        : {type: "int", value: parseStackBigInt(entry.value)}
    case "null":
      return {type: "null"}
    case "cell":
    case "slice":
    case "builder":
      return {type: entry.type, cell: Cell.fromBase64(extractStackBoc(entry.value, entry.type))}
    case "tuple":
    case "list": {
      if (!Array.isArray(entry.value)) {
        throw new TypeError(`${entry.type} stack value must be an array.`)
      }
      const items = entry.value.map(assertStackEntry).map(value => stackEntryToTupleItem(value))
      if (entry.type === "list" && items.length === 0) return {type: "null"}
      return {
        type: "tuple",
        items,
      }
    }
    case "nan":
      return {type: "nan"}
    default:
      throw new Error(`Unsupported runGetMethod stack type: ${entry.type}`)
  }
}

function parseStackBigInt(value: unknown): bigint {
  if (typeof value === "bigint") return value
  if (typeof value === "number" && Number.isSafeInteger(value)) return BigInt(value)
  if (typeof value === "string") {
    const normalized = value.trim()
    if (/^-?0x[0-9a-f]+$/i.test(normalized) || /^-?\d+$/.test(normalized)) {
      const negativeHex = normalized.match(/^-0x([0-9a-f]+)$/i)
      return negativeHex ? -BigInt(`0x${negativeHex[1]}`) : BigInt(normalized)
    }
  }
  throw new Error("Numeric stack value must be an integer or hex string.")
}

function assertStackEntry(value: unknown): AbiGetMethodStackEntry {
  if (!isRecord(value) || typeof value.type !== "string") {
    throw new Error("Nested stack entry must include string `type`.")
  }
  return {
    type: value.type,
    value: Object.hasOwn(value, "value") ? value.value : undefined,
  }
}

function extractStackBoc(value: unknown, type: string): string {
  if (typeof value === "string") return value
  if (isRecord(value) && typeof value.bytes === "string") return value.bytes
  throw new Error(`${type} stack value must be a base64 string or {bytes}.`)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value)
}

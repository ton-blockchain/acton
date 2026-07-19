import {beginCell, type Cell} from "@ton/core"
import {
  DynamicCtx,
  packToBuilderDynamic,
  renderTy,
  type ContractABI,
  type SymTable,
  unpackFromSliceDynamic,
} from "@ton/tolk-abi-to-typescript"

import {
  createAbiSymbols,
  normalizeAbiDynamicArg,
  parseAbiCellArg,
  parseAbiJsonStrict,
  sampleAbiValueForTy,
  stringifyAbiJson,
} from "./abiValue"

export interface AbiStorageBuilderInfo {
  readonly tyIdx: number
  readonly typeLabel: string
  readonly sampleJson: string
}

export function getAbiStorageBuilderInfo(
  abi: ContractABI | undefined,
): AbiStorageBuilderInfo | undefined {
  const tyIdx = abi?.storage?.storage_ty_idx
  if (abi === undefined || tyIdx === undefined) {
    return undefined
  }

  const symbols = createAbiSymbols(abi)
  return {
    tyIdx,
    typeLabel: safeRenderTy(symbols, tyIdx),
    sampleJson: stringifyAbiJson(sampleAbiValueForTy(symbols, tyIdx)),
  }
}

export function buildAbiStorageDataBoc(abi: ContractABI, storageJson: string): string {
  const tyIdx = abi.storage?.storage_ty_idx
  if (tyIdx === undefined) {
    throw new Error("ABI does not describe contract storage.")
  }

  return encodeAbiValueToBoc(abi, tyIdx, parseAbiJsonStrict(storageJson))
}

export function decodeAbiStorageDataBoc(abi: ContractABI, dataBoc: string): unknown {
  const tyIdx = abi.storage?.storage_ty_idx
  if (tyIdx === undefined) {
    throw new Error("ABI does not describe contract storage.")
  }

  return decodeAbiValueFromBoc(abi, tyIdx, dataBoc)
}

export function createAbiStorageSymbols(abi: ContractABI): SymTable {
  return createAbiSymbols(abi)
}

export function encodeAbiValueToCell(abi: ContractABI, tyIdx: number, formValue: unknown): Cell {
  const ctx = new DynamicCtx(abi)
  const normalizedInput = normalizeAbiDynamicArg(ctx, tyIdx, formValue)
  const builder = beginCell()
  packToBuilderDynamic(ctx, tyIdx, normalizedInput, builder)
  return builder.endCell()
}

export function encodeAbiValueToBoc(abi: ContractABI, tyIdx: number, formValue: unknown): string {
  return encodeAbiValueToCell(abi, tyIdx, formValue).toBoc().toString("hex")
}

export function decodeAbiValueFromCell(abi: ContractABI, tyIdx: number, cell: Cell): unknown {
  const ctx = new DynamicCtx(abi)
  return unpackFromSliceDynamic(ctx, tyIdx, cell.beginParse())
}

export function decodeAbiValueFromBoc(abi: ContractABI, tyIdx: number, dataBoc: string): unknown {
  return decodeAbiValueFromCell(abi, tyIdx, parseAbiCellArg(dataBoc))
}

function safeRenderTy(symbols: SymTable, tyIdx: number): string {
  try {
    return renderTy(symbols, tyIdx)
  } catch {
    return `ty#${tyIdx}`
  }
}

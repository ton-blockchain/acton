import {beginCell} from "@ton/core"
import {
  DynamicCtx,
  packToBuilderDynamic,
  renderTy,
  SymTable,
  type ContractABI,
  unpackFromSliceDynamic,
} from "@ton/tolk-abi-to-typescript"

import {
  normalizeAbiDynamicArg,
  parseAbiCellArg,
  sampleAbiValueForTy,
  stringifyAbiJson,
} from "../../../api/abiDynamic"

export interface AbiStorageBuilderInfo {
  readonly tyIdx: number
  readonly typeLabel: string
  readonly sampleJson: string
}

export function getAbiStorageBuilderInfo(abi: ContractABI | undefined): AbiStorageBuilderInfo | undefined {
  const tyIdx = abi?.storage?.storage_ty_idx
  if (abi === undefined || tyIdx === undefined) {
    return undefined
  }

  const symbols = createSymTable(abi)
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

  const ctx = new DynamicCtx(abi)
  const input = parseStorageJson(storageJson)
  const normalizedInput = normalizeAbiDynamicArg(ctx, tyIdx, input)
  const builder = beginCell()
  packToBuilderDynamic(ctx, tyIdx, normalizedInput, builder)
  return builder.endCell().toBoc().toString("base64")
}

export function decodeAbiStorageDataBoc(abi: ContractABI, dataBoc: string): unknown {
  const tyIdx = abi.storage?.storage_ty_idx
  if (tyIdx === undefined) {
    throw new Error("ABI does not describe contract storage.")
  }

  const ctx = new DynamicCtx(abi)
  return unpackFromSliceDynamic(ctx, tyIdx, parseAbiCellArg(dataBoc).beginParse())
}

export function createAbiStorageSymbols(abi: ContractABI): SymTable {
  return createSymTable(abi)
}

function createSymTable(abi: ContractABI): SymTable {
  return new SymTable(
    abi.declarations,
    abi.unique_types,
    abi.struct_instantiations,
    abi.alias_instantiations,
  )
}

function parseStorageJson(value: string): unknown {
  const trimmed = value.trim()
  return trimmed ? JSON.parse(trimmed) : {}
}

function safeRenderTy(symbols: SymTable, tyIdx: number): string {
  try {
    return renderTy(symbols, tyIdx)
  } catch {
    return `ty#${tyIdx}`
  }
}

import type {ABIStruct, ContractABI, Ty} from "@ton/tolk-abi-to-typescript"
import {SymTable} from "@ton/tolk-abi-to-typescript"

import {toRawAddress} from "../components/utils"

type CompilerAbiMessageKind = "incoming_messages" | "outgoing_messages"

export interface ContractAbiLink {
  readonly kind: string
  readonly title: string
  readonly url: string
  readonly scope: string
}

export interface ExtendedContractABI {
  readonly compiler_abi: ContractABI
  readonly display_name?: string
  readonly code_hashes: readonly string[]
  readonly links: readonly ContractAbiLink[]
}

export function addressKey(address: string): string {
  return toRawAddress(address)
}

export function buildMessageNamesByOpcodeHex(
  abi: ContractABI | null | undefined,
  messageKey: CompilerAbiMessageKind,
): Map<string, string> {
  const out = new Map<string, string>()
  for (const [opcode, name] of buildMessageNamesByOpcodeNumber(abi, messageKey)) {
    out.set(formatOpcode(opcode), name)
  }
  return out
}

export function buildMessageNamesByOpcodeNumber(
  abi: ContractABI | null | undefined,
  messageKey: CompilerAbiMessageKind,
): Map<number, string> {
  const out = new Map<number, string>()
  if (!abi) {
    return out
  }

  const symbols = new SymTable(
    abi.declarations,
    abi.unique_types,
    abi.struct_instantiations,
    abi.alias_instantiations,
  )

  for (const message of abi[messageKey] ?? []) {
    collectMessageEntries(message.body_ty_idx, symbols, out, new Set())
  }

  return out
}

function collectMessageEntries(
  bodyTyIdx: number,
  symbols: SymTable,
  out: Map<number, string>,
  visitedTyIdx: Set<number>,
): void {
  if (visitedTyIdx.has(bodyTyIdx)) {
    return
  }
  visitedTyIdx.add(bodyTyIdx)

  const bodyTy = tryTyByIdx(symbols, bodyTyIdx)
  const kind = bodyTy?.kind

  switch (kind) {
    case "StructRef": {
      const declaration = tryGetStruct(symbols, bodyTy.struct_name)
      const opcode = normalizeOpcodePrefix(
        declaration?.prefix?.prefix_num,
        declaration?.prefix?.prefix_len,
      )
      if (opcode !== undefined) {
        out.set(opcode, bodyTy.struct_name)
      }
      return
    }
    case "AliasRef": {
      const targetTyIdx = tryAliasTargetTyIdx(symbols, bodyTyIdx)
      if (targetTyIdx === undefined) {
        return
      }
      collectMessageEntries(targetTyIdx, symbols, out, visitedTyIdx)
      return
    }
    case "union": {
      for (const variant of bodyTy.variants) {
        const opcode = normalizeOpcodePrefix(variant.prefix_num, variant.prefix_len)
        const name = resolveMessageTypeName(variant.variant_ty_idx, symbols, new Set(visitedTyIdx))
        if (opcode !== undefined && name) {
          out.set(opcode, name)
        } else {
          collectMessageEntries(variant.variant_ty_idx, symbols, out, new Set(visitedTyIdx))
        }
      }
      return
    }
    case "nullable":
    case "cellOf":
    case "lispListOf": {
      collectMessageEntries(bodyTy.inner_ty_idx, symbols, out, visitedTyIdx)
      return
    }
    default: {
      return
    }
  }
}

function resolveMessageTypeName(
  bodyTyIdx: number,
  symbols: SymTable,
  visitedTyIdx: Set<number>,
): string | undefined {
  if (visitedTyIdx.has(bodyTyIdx)) {
    return undefined
  }
  visitedTyIdx.add(bodyTyIdx)

  const bodyTy = tryTyByIdx(symbols, bodyTyIdx)
  const kind = bodyTy?.kind

  switch (kind) {
    case "StructRef": {
      return bodyTy.struct_name
    }
    case "AliasRef": {
      const targetTyIdx = tryAliasTargetTyIdx(symbols, bodyTyIdx)
      if (targetTyIdx === undefined) {
        return undefined
      }
      return resolveMessageTypeName(targetTyIdx, symbols, visitedTyIdx)
    }
    case "nullable":
    case "cellOf":
    case "lispListOf": {
      return resolveMessageTypeName(bodyTy.inner_ty_idx, symbols, visitedTyIdx)
    }
    default: {
      return undefined
    }
  }
}

function tryTyByIdx(symbols: SymTable, tyIdx: number): Ty | undefined {
  try {
    return symbols.tyByIdx(tyIdx)
  } catch {
    return undefined
  }
}

function tryGetStruct(symbols: SymTable, structName: string): ABIStruct | undefined {
  try {
    return symbols.getStruct(structName)
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

function normalizeOpcodePrefix(prefixNum?: number, prefixLen?: number): number | undefined {
  if (prefixLen !== 32 || prefixNum === undefined) {
    return undefined
  }
  return prefixNum
}

function formatOpcode(opcode: number): string {
  return `0x${opcode.toString(16).padStart(8, "0")}`
}

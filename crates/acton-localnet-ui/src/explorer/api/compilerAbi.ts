import type {ContractABI} from "gen-typescript-from-tolk-dev"

import {parseAddress} from "../components/utils"

type CompilerAbiMessageKind = "incoming_messages" | "outgoing_messages"

export function addressKey(address: string): string {
  const parsed = parseAddress(address)
  const rawString = (parsed as {toRawString?: () => string} | undefined)?.toRawString
  return typeof rawString === "function" ? rawString.call(parsed) : address
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

  const structDeclarations = new Map<string, Record<string, unknown>>()
  const aliasDeclarations = new Map<string, Record<string, unknown>>()

  for (const declaration of abi.declarations ?? []) {
    const item = asRecord(declaration)
    const kind = typeof item?.kind === "string" ? item.kind : undefined
    const name = typeof item?.name === "string" ? item.name : undefined
    if (!item || !kind || !name) {
      continue
    }
    if (kind === "struct") {
      structDeclarations.set(name, item)
    } else if (kind === "alias") {
      aliasDeclarations.set(name, item)
    }
  }

  for (const message of abi[messageKey] ?? []) {
    const bodyTy = asRecord(asRecord(message)?.body_ty)
    if (!bodyTy) {
      continue
    }
    collectMessageEntries(bodyTy, structDeclarations, aliasDeclarations, out, new Set())
  }

  return out
}

function collectMessageEntries(
  bodyTy: Record<string, unknown>,
  structDeclarations: Map<string, Record<string, unknown>>,
  aliasDeclarations: Map<string, Record<string, unknown>>,
  out: Map<number, string>,
  visitedAliases: Set<string>,
): void {
  const kind = typeof bodyTy.kind === "string" ? bodyTy.kind : undefined
  if (!kind) {
    return
  }

  switch (kind) {
    case "StructRef": {
      const structName = typeof bodyTy.struct_name === "string" ? bodyTy.struct_name : undefined
      if (!structName) {
        return
      }
      const declaration = structDeclarations.get(structName)
      const prefix = asRecord(declaration?.prefix)
      const opcode = normalizeOpcodePrefix(
        typeof prefix?.prefix_str === "string" ? prefix.prefix_str : undefined,
        typeof prefix?.prefix_len === "number" ? prefix.prefix_len : undefined,
      )
      if (opcode !== undefined) {
        out.set(opcode, structName)
      }
      return
    }
    case "AliasRef": {
      const aliasName = typeof bodyTy.alias_name === "string" ? bodyTy.alias_name : undefined
      if (!aliasName || visitedAliases.has(aliasName)) {
        return
      }
      visitedAliases.add(aliasName)
      const declaration = aliasDeclarations.get(aliasName)
      const targetTy = asRecord(declaration?.target_ty)
      if (declaration && targetTy) {
          collectMessageEntries(targetTy, structDeclarations, aliasDeclarations, out, visitedAliases)
        }
      visitedAliases.delete(aliasName)
      return
    }
    case "union": {
      const variants = Array.isArray(bodyTy.variants) ? bodyTy.variants : []
      for (const variant of variants) {
        const item = asRecord(variant)
        const variantTy = asRecord(item?.variant_ty)
        if (!item || !variantTy) {
          continue
        }
        const opcode = normalizeOpcodePrefix(
          typeof item.prefix_str === "string" ? item.prefix_str : undefined,
          typeof item.prefix_len === "number" ? item.prefix_len : undefined,
        )
        const name = resolveMessageTypeName(variantTy, aliasDeclarations, new Set(visitedAliases))
        if (opcode !== undefined && name) {
          out.set(opcode, name)
        } else {
          collectMessageEntries(
            variantTy,
            structDeclarations,
            aliasDeclarations,
            out,
            new Set(visitedAliases),
          )
        }
      }
      return
    }
    case "nullable":
    case "cellOf":
    case "lispListOf": {
      const inner = asRecord(bodyTy.inner)
      if (!inner) {
        return
      }
      collectMessageEntries(
        inner,
        structDeclarations,
        aliasDeclarations,
        out,
        visitedAliases,
      )
      return
    }
    default: {
      return
    }
  }
}

function resolveMessageTypeName(
  bodyTy: Record<string, unknown>,
  aliasDeclarations: Map<string, Record<string, unknown>>,
  visitedAliases: Set<string>,
): string | undefined {
  const kind = typeof bodyTy.kind === "string" ? bodyTy.kind : undefined
  if (!kind) {
    return undefined
  }

  switch (kind) {
    case "StructRef": {
      return typeof bodyTy.struct_name === "string" ? bodyTy.struct_name : undefined
    }
    case "AliasRef": {
      const aliasName = typeof bodyTy.alias_name === "string" ? bodyTy.alias_name : undefined
      if (!aliasName || visitedAliases.has(aliasName)) {
        return undefined
      }
      visitedAliases.add(aliasName)
      const declaration = aliasDeclarations.get(aliasName)
      const targetTy = asRecord(declaration?.target_ty)
      return targetTy ? resolveMessageTypeName(targetTy, aliasDeclarations, visitedAliases) : undefined
    }
    case "nullable":
    case "cellOf":
    case "lispListOf": {
      const inner = asRecord(bodyTy.inner)
      return inner ? resolveMessageTypeName(inner, aliasDeclarations, visitedAliases) : undefined
    }
    default: {
      return undefined
    }
  }
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" ? (value as Record<string, unknown>) : undefined
}

function normalizeOpcodePrefix(prefix?: string, prefixLen?: number): number | undefined {
  if (!prefix || prefixLen !== 32) {
    return undefined
  }
  return parseOpcode(prefix)
}

function parseOpcode(opcode: string): number | undefined {
  const normalized = opcode.trim()
  if (!normalized) {
    return undefined
  }

  try {
    const value =
      normalized.startsWith("0x") || normalized.startsWith("0X")
        ? Number.parseInt(normalized.slice(2), 16)
        : Number.parseInt(normalized, 10)

    if (!Number.isInteger(value) || value < 0 || value > 0xff_ff_ff_ff) {
      return undefined
    }

    return value
  } catch {
    return undefined
  }
}

function formatOpcode(opcode: number): string {
  return `0x${opcode.toString(16).padStart(8, "0")}`
}

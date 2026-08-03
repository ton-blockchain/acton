import {decodeCellWithAbi} from "@acton/transaction-ui"
import type {Cell} from "@ton/core"

import type {ExtendedContractABI} from "../api/compilerAbi"
import type {ParserWarning} from "./model"

export interface AbiInferenceCandidate {
  readonly abi: ExtendedContractABI
}

export interface AbiInferenceResult {
  readonly abi?: ExtendedContractABI
  readonly warning?: ParserWarning
  readonly confidenceScore?: number
  readonly confidenceReason?: string
}

export function inferAbiByOpcode(
  cell: Cell,
  candidates: readonly AbiInferenceCandidate[],
): AbiInferenceResult {
  if (cell.bits.length < 32) return {}

  const opcode = cell.beginParse().loadUint(32)
  const groups = new Map<
    string,
    {readonly candidate: AbiInferenceCandidate; readonly name: string; readonly count: number}
  >()
  let completeMatches = 0

  for (const candidate of candidates) {
    if (!compilerAbiDeclaresOpcode(candidate.abi, opcode)) continue
    const decoded = decodeCellWithAbi(cell, candidate.abi)
    if (!decoded || decoded.category === "comment" || decoded.consumption?.complete !== true) {
      continue
    }

    completeMatches += 1
    const key = `${decoded.name}\u0000${JSON.stringify(decoded.value)}`
    const current = groups.get(key)
    groups.set(key, {
      candidate: current?.candidate ?? candidate,
      name: decoded.name,
      count: (current?.count ?? 0) + 1,
    })
  }

  const ranked = [...groups.values()].toSorted(
    (left, right) => right.count - left.count || left.name.localeCompare(right.name),
  )
  const selected = ranked[0]
  if (!selected || (ranked[1]?.count ?? 0) === selected.count) return {}

  const opcodeLabel = `0x${opcode.toString(16).padStart(8, "0")}`
  return {
    abi: {...selected.candidate.abi, display_name: "ABI catalog"},
    confidenceScore: 0.7,
    confidenceReason: `No contract was identified. ${selected.count} ABI definitions agree on ${selected.name} for ${opcodeLabel}`,
    warning: {
      code: "ambiguous-match",
      message: `${completeMatches} ABI definitions decode ${opcodeLabel}, with ${ranked.length} possible layouts. Showing the most common ${selected.name} layout`,
    },
  }
}

function compilerAbiDeclaresOpcode(abi: ExtendedContractABI, opcode: number): boolean {
  return abi.compiler_abi.declarations.some(declaration => {
    if (declaration.kind !== "struct") return false
    return declaration.prefix?.prefix_len === 32 && Number(declaration.prefix.prefix_num) === opcode
  })
}

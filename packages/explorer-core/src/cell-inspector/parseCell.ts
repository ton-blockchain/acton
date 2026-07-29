import type {ParsedValue} from "@acton/ui"
import {decodeCellWithAbi, type ExtendedContractABI} from "@acton/transaction-ui"
import type {Cell} from "@ton/core"

import {parseBlockTlb} from "./blockParser"
import {tryParseCustomTlb} from "./customTlb"
import {decodeCellInput} from "./inputNormalization"
import {confidence, type CellParseResult, type ParserWarning} from "./model"
import {describeCellForest} from "./rawCellTree"
import {toSerializable} from "./serializable"
import {recognizeStandardComment} from "./standardComments"

export interface CellInspectorParseOptions {
  readonly rootIndex: number
  readonly strict: boolean
  readonly maxDepth: number
  readonly customTlb: string
  readonly customTlbAuthoritative?: boolean
  readonly abi?: ExtendedContractABI
  readonly abiCodeHash?: string
  /** Report a failed ABI decode when the ABI was explicitly selected by the user. */
  readonly warnOnAbiMismatch?: boolean
  readonly abiConfidence?: {
    readonly score: number
    readonly reason: string
  }
}

export type CellInspectorParseResult = CellParseResult & {
  readonly abiValue?: ParsedValue
  readonly selectedRootBocHex?: string
  readonly bocHex?: string
  readonly bocBase64?: string
}

export interface CellHashCandidate {
  readonly hash: string
  readonly path: string
}

const MAX_HASH_CANDIDATES = 16

export function collectCellHashCandidates(
  roots: readonly Cell[],
  maxNodes = MAX_HASH_CANDIDATES,
): readonly CellHashCandidate[] {
  const candidates: CellHashCandidate[] = []
  const visited = new Set<string>()
  const queue = roots.map((cell, index) => ({cell, path: `$[${index}]`}))

  while (queue.length > 0 && candidates.length < maxNodes) {
    const current = queue.shift()
    if (!current) break

    const hash = Buffer.from(current.cell.hash()).toString("hex")
    if (visited.has(hash)) continue
    visited.add(hash)
    candidates.push({hash, path: current.path})

    current.cell.refs.forEach((cell, index) => {
      queue.push({cell, path: `${current.path}.refs[${index}]`})
    })
  }

  return candidates
}

export function parseCell(
  input: string,
  options: CellInspectorParseOptions,
): CellInspectorParseResult {
  const decodedResult = decodeCellInput(input, {rootIndex: options.rootIndex})
  if (!decodedResult.ok) {
    return {
      status: "error",
      error: decodedResult.error,
      warnings: [],
      ...(decodedResult.normalized === undefined ? {} : {normalized: decodedResult.normalized}),
    }
  }

  const {decoded} = decodedResult
  const {selectedRoot} = decoded
  const rawForest = describeCellForest([selectedRoot], {
    maxDepth: options.maxDepth,
    includeBoc: false,
  })
  const raw = toSerializable(rawForest)
  const cell = {
    bits: selectedRoot.bits.length,
    refs: selectedRoot.refs.length,
    depth: selectedRoot.depth(),
    hash: Buffer.from(selectedRoot.hash()).toString("hex"),
    rootIndex: decoded.selectedRootIndex,
    rootCount: decoded.roots.length,
  }
  const selectedRootBocHex = selectedRoot.toBoc().toString("hex")
  const originalBytes =
    decoded.normalized.kind === "hex"
      ? Buffer.from(decoded.normalized.value, "hex")
      : Buffer.from(decoded.normalized.value, "base64")
  const shared = {
    normalized: decoded.normalized,
    cell,
    raw,
    selectedRootBocHex,
    bocHex: originalBytes.toString("hex"),
    bocBase64: originalBytes.toString("base64"),
  }
  const accumulatedWarnings: ParserWarning[] = [...rawForest.warnings]
  const parseCustomTlb = (): CellInspectorParseResult | undefined => {
    const custom = tryParseCustomTlb(selectedRoot, options.customTlb)
    if (custom?.matched) {
      const warnings = [...accumulatedWarnings, ...custom.warnings]
      return {
        status: warnings.length > 0 ? "partial" : "success",
        parser: "custom-tlb",
        data: custom.data,
        provenance: custom.provenance,
        warnings,
        ...shared,
      }
    }
    if (custom && !custom.matched) accumulatedWarnings.push(...custom.warnings)
    return undefined
  }

  if (options.customTlbAuthoritative) {
    const customResult = parseCustomTlb()
    if (customResult) return customResult
    return {
      status: "error",
      error: {
        code: "custom-tlb-failed",
        message: "Custom TL-B could not decode this root",
        cause:
          accumulatedWarnings.at(-1)?.message ?? "Enter a TL-B schema before inspecting this root",
      },
      warnings: accumulatedWarnings,
      ...shared,
    }
  }

  const comment = recognizeStandardComment(selectedRoot)
  if (comment) {
    const warnings = [...accumulatedWarnings, ...comment.warnings]
    const {provenance, warnings: _commentWarnings, ...commentData} = comment
    return {
      status: warnings.length > 0 ? "partial" : "success",
      parser: "standard-comment",
      data: toSerializable(commentData),
      provenance,
      warnings,
      ...shared,
    }
  }

  if (options.abi) {
    const decodedAbi = decodeCellWithAbi(selectedRoot, options.abi)
    const abiConsumptionComplete = decodedAbi?.consumption?.complete !== false
    if (
      decodedAbi &&
      decodedAbi.category !== "comment" &&
      (!options.strict || abiConsumptionComplete)
    ) {
      const consumptionWarnings =
        decodedAbi.consumption?.complete === false
          ? [
              {
                code: "partial-match" as const,
                message: `This ABI decoded the value but left ${decodedAbi.consumption.remainingBits} bits and ${decodedAbi.consumption.remainingRefs} references unread`,
              },
            ]
          : []
      const directionWarnings =
        decodedAbi.directionCandidates && decodedAbi.directionCandidates.length > 1
          ? [
              {
                code: "ambiguous-match" as const,
                message: `This value matches more than one ABI message direction: ${decodedAbi.directionCandidates.join(", ")}`,
              },
            ]
          : []
      const warnings = [...accumulatedWarnings, ...consumptionWarnings, ...directionWarnings]
      const label = options.abi.display_name ?? options.abi.compiler_abi.contract_name
      const details: Record<string, string> = {
        value: decodedAbi.name,
        category: decodedAbi.category,
      }
      if (decodedAbi.direction) details.direction = decodedAbi.direction
      if (decodedAbi.directionCandidates) {
        details.directionCandidates = decodedAbi.directionCandidates.join(", ")
      }
      if (options.abiCodeHash) details.codeHash = options.abiCodeHash
      const hasPartialConsumption = decodedAbi.consumption?.complete === false
      const hasAmbiguousDirection = directionWarnings.length > 0
      const decodedConfidenceScore = hasAmbiguousDirection
        ? 0.84
        : hasPartialConsumption
          ? 0.9
          : 0.99
      const confidenceScore = Math.min(
        decodedConfidenceScore,
        options.abiConfidence?.score ?? decodedConfidenceScore,
      )
      const confidenceReason = hasAmbiguousDirection
        ? "The ABI decoded the value, but its message direction is ambiguous"
        : hasPartialConsumption
          ? "The ABI decoded the value with unread data remaining"
          : "The ABI used the entire root cell"

      return {
        status: warnings.length > 0 ? "partial" : "success",
        parser: "abi-registry",
        data: toSerializable(decodedAbi.value),
        abiValue: decodedAbi.value,
        provenance: {
          engine: "abi-registry",
          label: label ? `${label} · ${decodedAbi.name}` : decodedAbi.name,
          source: "abi-registry",
          confidence: confidence(confidenceScore, [
            confidenceReason,
            ...(options.abiConfidence ? [options.abiConfidence.reason] : []),
          ]),
          details,
        },
        warnings,
        ...shared,
      }
    }
    if (!decodedAbi && options.warnOnAbiMismatch !== false) {
      accumulatedWarnings.push({
        code: "abi-context-unavailable",
        message: "This ABI does not match the selected root cell",
      })
    } else if (options.strict && !abiConsumptionComplete) {
      accumulatedWarnings.push({
        code: "partial-match",
        message: `Strict parsing ignored this ABI because ${decodedAbi.consumption?.remainingBits ?? 0} bits and ${decodedAbi.consumption?.remainingRefs ?? 0} references remained unread`,
      })
    }
  }

  const customResult = parseCustomTlb()
  if (customResult) return customResult

  const block = parseBlockTlb(selectedRoot, {strict: options.strict})
  if (block) {
    const warnings = [
      ...accumulatedWarnings,
      ...block.warnings.map(message => ({
        code: message.startsWith("This root also matches")
          ? ("ambiguous-match" as const)
          : ("partial-match" as const),
        message,
      })),
    ]
    return {
      status: warnings.length > 0 ? "partial" : "success",
      parser: "block-tlb",
      data: toSerializable(block.value),
      provenance: {
        engine: "block-tlb",
        label: `TON block.tlb · ${block.name}`,
        source: "canonical-block-tlb",
        confidence: confidence(block.confidence, [
          block.consumption.complete
            ? "This TON block format used the entire root cell"
            : `This TON block format used ${Math.round(block.consumption.coverage * 100)}% of the root cell`,
        ]),
        details: {
          type: block.name,
          ...consumptionDetails(block.consumption),
        },
      },
      warnings,
      ...shared,
    }
  }

  return {
    status: "unknown",
    parser: "raw-cell-tree",
    data: toSerializable(rawForest.roots[0] ?? rawForest),
    provenance: {
      engine: "raw-cell-tree",
      label: "Raw cell structure",
      source: "fallback",
      confidence: confidence(0.3, ["No known ABI or TON format matched this root cell"]),
    },
    warnings: accumulatedWarnings,
    ...shared,
  }
}

function consumptionDetails(consumption: {
  readonly consumedBits: number
  readonly consumedRefs: number
  readonly remainingBits: number
  readonly remainingRefs: number
}): Record<string, string> {
  return {
    consumedBits: String(consumption.consumedBits),
    consumedRefs: String(consumption.consumedRefs),
    remainingBits: String(consumption.remainingBits),
    remainingRefs: String(consumption.remainingRefs),
  }
}

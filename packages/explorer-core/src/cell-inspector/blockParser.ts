import type {Cell, Slice} from "@ton/core"

import {
  loadAccount,
  loadAccountBlock,
  loadBlock,
  loadBlockExtra,
  loadBlockInfo,
  loadConfigParams,
  loadCurrencyCollection,
  loadInMsg,
  loadInMsgDescr,
  loadMessageAny,
  loadMsgEnvelope,
  loadOutAction,
  loadOutList,
  loadOutMsg,
  loadOutMsgDescr,
  loadShardAccount,
  loadShardState,
  loadShardStateUnsplit,
  loadStateInit,
  loadTransaction,
  loadTransactionDescr,
  loadValueFlow,
} from "./block.tlb.generated"

export type CanonicalBlockTlbName =
  | "Block"
  | "ShardState"
  | "ShardStateUnsplit"
  | "BlockInfo"
  | "BlockExtra"
  | "ValueFlow"
  | "Transaction"
  | "TransactionDescr"
  | "AccountBlock"
  | "MessageAny"
  | "MsgEnvelope"
  | "OutList"
  | "OutAction"
  | "ShardAccount"
  | "Account"
  | "StateInit"
  | "InMsg"
  | "InMsgDescr"
  | "OutMsg"
  | "OutMsgDescr"
  | "ConfigParams"
  | "CurrencyCollection"

export interface SliceConsumption {
  readonly initialBits: number
  readonly initialRefs: number
  readonly consumedBits: number
  readonly consumedRefs: number
  readonly remainingBits: number
  readonly remainingRefs: number
  readonly complete: boolean
  /** Normalized root-slice coverage in the 0..1 range. */
  readonly coverage: number
}

export interface BlockTlbParseResult {
  readonly parser: "block.tlb"
  readonly name: CanonicalBlockTlbName
  readonly value: unknown
  /** BoC of the input slice, encoded as lowercase hex. */
  readonly boc?: string
  readonly consumption: SliceConsumption
  readonly warnings: readonly string[]
  readonly confidence: number
}

export interface BlockTlbParseOptions {
  /** Only accept a loader when it consumes every bit and reference of the root slice. */
  readonly strict?: boolean
}

export interface BlockMetadata {
  readonly genSoftwareVersion?: number
  readonly genSoftwareCapabilities?: bigint
  readonly feesCollected: bigint
}

interface CanonicalLoader {
  readonly name: CanonicalBlockTlbName
  readonly load: (slice: Slice) => unknown
  readonly completeConfidence: number
}

interface LoaderCandidate extends BlockTlbParseResult {
  readonly priority: number
}

// More structurally distinctive top-level types intentionally run first. This also
// makes the stable declaration order the tie-breaker for equally complete parses.
const CANONICAL_LOADERS: readonly CanonicalLoader[] = [
  {name: "Block", load: loadBlock, completeConfidence: 0.99},
  {name: "ShardState", load: loadShardState, completeConfidence: 0.98},
  {name: "ShardStateUnsplit", load: loadShardStateUnsplit, completeConfidence: 0.98},
  {name: "BlockInfo", load: loadBlockInfo, completeConfidence: 0.96},
  {name: "BlockExtra", load: loadBlockExtra, completeConfidence: 0.95},
  {name: "ValueFlow", load: loadValueFlow, completeConfidence: 0.94},
  {name: "Transaction", load: loadTransaction, completeConfidence: 0.96},
  {name: "TransactionDescr", load: loadTransactionDescr, completeConfidence: 0.93},
  {name: "AccountBlock", load: loadAccountBlock, completeConfidence: 0.94},
  {name: "MessageAny", load: loadMessageAny, completeConfidence: 0.92},
  {name: "MsgEnvelope", load: loadMsgEnvelope, completeConfidence: 0.91},
  {name: "OutList", load: loadUnknownLengthOutList, completeConfidence: 0.95},
  {name: "OutAction", load: loadOutAction, completeConfidence: 0.95},
  {name: "ShardAccount", load: loadShardAccount, completeConfidence: 0.95},
  {name: "Account", load: loadAccount, completeConfidence: 0.86},
  // StateInit has no constructor prefix and consists mostly of optional fields, so a
  // complete decode is only a weak structural hypothesis without surrounding context.
  {name: "StateInit", load: loadStateInit, completeConfidence: 0.55},
  {name: "InMsg", load: loadInMsg, completeConfidence: 0.9},
  {name: "InMsgDescr", load: loadInMsgDescr, completeConfidence: 0.9},
  {name: "OutMsg", load: loadOutMsg, completeConfidence: 0.9},
  {name: "OutMsgDescr", load: loadOutMsgDescr, completeConfidence: 0.9},
  {name: "ConfigParams", load: loadConfigParams, completeConfidence: 0.88},
  {name: "CurrencyCollection", load: loadCurrencyCollection, completeConfidence: 0.78},
]

function asSlice(input: Cell | Slice): Slice {
  return "beginParse" in input ? input.beginParse(true) : input.clone()
}

function getBoc(input: Cell | Slice): string {
  return ("toBoc" in input ? input : input.asCell()).toBoc().toString("hex")
}

export function parseBlockMetadata(input: Cell | Slice): BlockMetadata {
  const block = loadBlock(asSlice(input))
  return {
    genSoftwareVersion: block.info.gen_software?.version,
    genSoftwareCapabilities: block.info.gen_software?.capabilities,
    feesCollected: block.value_flow.fees_collected.grams,
  }
}

function getCoverage(
  initialBits: number,
  initialRefs: number,
  remainingBits: number,
  remainingRefs: number,
): number {
  const dimensions: number[] = []

  if (initialBits > 0) {
    dimensions.push((initialBits - remainingBits) / initialBits)
  }
  if (initialRefs > 0) {
    dimensions.push((initialRefs - remainingRefs) / initialRefs)
  }

  if (dimensions.length === 0) {
    return 1
  }

  return dimensions.reduce((sum, value) => sum + value, 0) / dimensions.length
}

function getConsumption(initial: Slice, parsed: Slice): SliceConsumption {
  const initialBits = initial.remainingBits
  const initialRefs = initial.remainingRefs
  const remainingBits = parsed.remainingBits
  const remainingRefs = parsed.remainingRefs

  return {
    initialBits,
    initialRefs,
    consumedBits: initialBits - remainingBits,
    consumedRefs: initialRefs - remainingRefs,
    remainingBits,
    remainingRefs,
    complete: remainingBits === 0 && remainingRefs === 0,
    coverage: getCoverage(initialBits, initialRefs, remainingBits, remainingRefs),
  }
}

function getConfidence(loader: CanonicalLoader, consumption: SliceConsumption): number {
  if (consumption.complete) {
    return loader.completeConfidence
  }

  // A partial parse is useful as a hypothesis, but it must remain visibly less
  // trustworthy than any complete parse, even when it consumed almost everything.
  return Math.min(0.74, loader.completeConfidence * (0.35 + consumption.coverage * 0.45))
}

function compareCandidates(left: LoaderCandidate, right: LoaderCandidate): number {
  if (left.consumption.complete !== right.consumption.complete) {
    return left.consumption.complete ? -1 : 1
  }

  if (left.consumption.coverage !== right.consumption.coverage) {
    return right.consumption.coverage - left.consumption.coverage
  }

  if (left.consumption.consumedRefs !== right.consumption.consumedRefs) {
    return right.consumption.consumedRefs - left.consumption.consumedRefs
  }

  if (left.consumption.consumedBits !== right.consumption.consumedBits) {
    return right.consumption.consumedBits - left.consumption.consumedBits
  }

  return left.priority - right.priority
}

function loadUnknownLengthOutList(slice: Slice): unknown {
  let length = 0
  let current = slice.clone()

  while (current.remainingBits > 0 || current.remainingRefs > 0) {
    if (current.remainingBits < 32 || current.remainingRefs < 1 || length >= 256) {
      throw new Error("Invalid or excessively deep OutList")
    }

    current = current.loadRef().beginParse(true)
    length += 1
  }

  if (length === 0) {
    throw new Error("An empty cell is too ambiguous to classify as OutList")
  }

  return loadOutList(slice, length)
}

/**
 * Tries the canonical top-level loaders generated from TON's `block.tlb`.
 *
 * Each loader gets its own clone, because a failed TL-B loader may have already
 * advanced the slice. In relaxed mode the most complete successful hypothesis is
 * returned; strict mode rejects every candidate that leaves root data unread.
 */
export function parseBlockTlb(
  input: Cell | Slice,
  options: BlockTlbParseOptions = {},
): BlockTlbParseResult | undefined {
  const source = asSlice(input)
  const boc = getBoc(input)
  const candidates: LoaderCandidate[] = []

  for (const [priority, loader] of CANONICAL_LOADERS.entries()) {
    const candidateSlice = source.clone()

    try {
      const value = loader.load(candidateSlice)
      const consumption = getConsumption(source, candidateSlice)

      if (options.strict && !consumption.complete) {
        continue
      }

      const warnings = consumption.complete
        ? []
        : [
            `${loader.name} decoded only part of the root cell: ${consumption.remainingBits} bits and ${consumption.remainingRefs} references remain`,
          ]

      candidates.push({
        parser: "block.tlb",
        name: loader.name,
        value,
        boc,
        consumption,
        warnings,
        confidence: getConfidence(loader, consumption),
        priority,
      })
    } catch {
      // A loader mismatch is expected while probing alternative top-level schemas.
    }
  }

  candidates.sort(compareCandidates)
  const selected = candidates[0]

  if (!selected) {
    return undefined
  }

  const {priority: _priority, ...result} = selected
  const equallyComplete = candidates
    .slice(1)
    .filter(
      candidate =>
        candidate.consumption.complete === selected.consumption.complete &&
        candidate.consumption.coverage === selected.consumption.coverage,
    )

  if (equallyComplete.length === 0) {
    return result
  }

  return {
    ...result,
    warnings: [
      ...result.warnings,
      `This root also matches: ${equallyComplete.map(candidate => candidate.name).join(", ")}`,
    ],
    confidence: Math.max(0, result.confidence - 0.08),
  }
}

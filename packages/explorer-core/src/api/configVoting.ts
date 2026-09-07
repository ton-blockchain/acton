import {Address, Cell, Dictionary, loadShardAccount} from "@ton/core"

import {
  loadConfigProposal,
  loadConfigProposalStatus,
  loadConfigVotingSetup,
} from "../cell-inspector/block.tlb.generated"
import type {ConfigProposalSetup, ConfigProposalStatus} from "../cell-inspector/block.tlb.generated"
import type {TonClient} from "./client"
import {parseConfigParameter, parseNetworkConfig} from "./config"
import type {NetworkConfig, NetworkConfigParameter} from "./config"
import type {V3Transaction} from "./types"

const REGISTER_PROPOSAL = 0x6e_56_50_52
const REGISTERED = 0xee_56_50_52
const ACCEPTED_NORMAL = 0xd6_74_52_46
const ACCEPTED_CRITICAL = 0xd6_74_52_47

/** A proposal is identified by its cell hash; a registration can extend an existing proposal. */
export interface ConfigVote {
  readonly hash: string
  readonly parameterId: number
  readonly proposed?: NetworkConfigParameter
  readonly requiredHash?: string
  readonly critical: boolean
  readonly expires?: number
  readonly registeredAt?: number
  readonly registrationSeqno?: number
  readonly transactionHash?: string
  readonly state?: ConfigProposalStatus
}

/** All values in a snapshot come from one masterchain state, including the voting rules. */
export interface ConfigVotingSnapshot {
  readonly seqno: number
  readonly time: number
  readonly address: string
  readonly config: NetworkConfig
  readonly proposals: readonly ConfigVote[]
  readonly rules: {readonly normal: ConfigProposalSetup; readonly critical: ConfigProposalSetup}
}

/** A missing proposal is only “closed” until an archival transition proves its outcome. */
export interface ConfigVoteResult {
  readonly outcome: "accepted" | "passed" | "expired" | "rejected" | "closed"
  readonly time: number
  readonly seqno: number
  readonly vote: ConfigVote
  readonly snapshot: ConfigVotingSnapshot
  readonly finalVoteRecorded: boolean
  readonly removed: boolean
}

/** Resolves weights against the set recorded in the proposal, never an unrelated current set. */
export function configVoteTally(vote: ConfigVote, snapshot: ConfigVotingSnapshot) {
  const state = vote.state
  if (!state) return undefined

  const validatorParameter = snapshot.config.parameters.find(
    parameter =>
      [32, 34, 36].includes(parameter.id) &&
      cellFromParameter(parameter).hash().toString("hex") === hexHash(state.validator_set_id),
  )
  const set = validatorParameter?.validatorSet
  if (!set) return undefined

  const total =
    set.totalWeight ?? set.validators.reduce((sum, validator) => sum + validator.weight, 0n)
  if (total === 0n) return undefined

  const threshold = (total * 3n) / 4n
  const voted = threshold - state.remaining_weight
  const voters = new Set(state.voters.keys())
  const remaining = state.remaining_weight < 0n ? 0n : state.remaining_weight + 1n

  // A round requires strictly more than three quarters of the total weight
  // Count the largest remaining validators first to obtain the minimum count
  let covered = 0n
  let neededValidators = 0

  for (const validator of [...set.validators]
    .filter(validator => !voters.has(validator.index))
    .sort((a, b) => (a.weight > b.weight ? -1 : a.weight < b.weight ? 1 : 0))) {
    if (covered >= remaining) break
    covered += validator.weight
    neededValidators += 1
  }

  return {
    set,
    voted,
    total,
    percent: weightPercent(voted, total),
    remainingPercent: weightPercent(remaining, total),
    neededValidators,
    won: state.remaining_weight < 0n,
    currentSet: validatorParameter.id === 34,
  }
}

/** Ratios are rounded only for display; threshold decisions use integer weights. */
export function weightPercent(weight: bigint, total: bigint): number {
  return total > 0n ? Number((weight * 1_000_000n) / total) / 10_000 : 0
}

function hexHash(value: bigint) {
  return value.toString(16).padStart(64, "0")
}

function cellFromParameter(parameter: NetworkConfigParameter) {
  return Cell.fromBoc(Buffer.from(parameter.rawHex, "hex"))[0]
}

function readProposal(cell: Cell, critical: boolean): ConfigVote {
  const value = loadConfigProposal(cell.beginParse())

  return {
    hash: cell.hash().toString("hex"),
    parameterId: value.param_id,
    proposed:
      value.param_value.kind === "Maybe_just"
        ? parseConfigParameter(value.param_id, value.param_value.value)
        : undefined,
    requiredHash:
      value.if_hash_equal.kind === "Maybe_just" ? hexHash(value.if_hash_equal.value) : undefined,
    critical,
  }
}

/** Reads config-code.fc storage directly, avoiding the deployed get-method's swapped win/loss fields. */
export function parseConfigVotingState(
  data: Cell,
  address: string,
  seqno: number,
  time: number,
): ConfigVotingSnapshot {
  const slice = data.beginParse()
  const config = parseNetworkConfig(slice.loadRef().toBoc().toString("base64"))
  slice.skip(32 + 256)

  const proposals = slice.loadDict(Dictionary.Keys.BigUint(256), {
    serialize: () => {
      throw new Error("Voting state is read-only")
    },
    parse: proposalSlice => {
      // The deployed contract can store vote timestamps in dictionary leaves
      // declared as True in block.tlb; the generated loader reads their keys.
      const proposalCell = proposalSlice.preloadRef()
      const state = loadConfigProposalStatus(proposalSlice)
      return {...readProposal(proposalCell, state.is_critical.value), expires: state.expires, state}
    },
  })
  slice.endParse()

  const votingParameter = config.parameters.find(parameter => parameter.id === 11)
  if (!votingParameter) throw new Error("Network configuration has no voting rules")
  const rules = loadConfigVotingSetup(cellFromParameter(votingParameter).beginParse())

  return {
    address,
    seqno,
    time,
    config,
    proposals: [...proposals].map(([hash, proposal]) => {
      if (hexHash(hash) !== proposal.hash)
        throw new Error("Proposal hash does not match its dictionary key")
      return proposal
    }),
    rules: {normal: rules.normal_params, critical: rules.critical_params},
  }
}

function sameAddress(left: string, right: string) {
  return Address.parse(left).equals(Address.parse(right))
}

function successful(transaction: V3Transaction) {
  return !transaction.description.aborted && !transaction.in_msg?.bounced
}

function hasResponse(transaction: V3Transaction, opcode: number) {
  return transaction.out_msgs.some(message => Number(message.opcode) === opcode)
}

/** Only a successful registration receipt creates history; failed submissions are excluded. */
export function configVoteRegistrations(transactions: readonly V3Transaction[], address: string) {
  const proposals: ConfigVote[] = []

  for (const transaction of transactions) {
    if (
      !sameAddress(transaction.account, address) ||
      !successful(transaction) ||
      !hasResponse(transaction, REGISTERED)
    ) {
      continue
    }

    const body = transaction.in_msg?.message_content.body
    if (!body) throw new Error("Registered proposal transaction has no message body")

    const slice = Cell.fromBase64(body).beginParse()
    if (slice.loadUint(32) !== REGISTER_PROPOSAL) continue
    slice.skip(64 + 32)
    const proposal = slice.loadRef()

    proposals.push({
      ...readProposal(proposal, slice.loadBoolean()),
      registeredAt: transaction.now,
      registrationSeqno: transaction.mc_block_seqno,
      transactionHash: transaction.hash,
    })
  }

  return proposals
}

function acceptedHash(transaction: V3Transaction): {hash: string; index: number} | undefined {
  const body = transaction.in_msg?.message_content.body
  if (
    !body ||
    !successful(transaction) ||
    (!hasResponse(transaction, ACCEPTED_NORMAL) && !hasResponse(transaction, ACCEPTED_CRITICAL))
  ) {
    return undefined
  }

  const slice = Cell.fromBase64(body).beginParse()
  if (slice.loadUint(32) !== 0x56_6f_74_65) return undefined
  slice.skip(64 + 512)
  if (slice.loadUint(32) !== 0x56_6f_74_45) return undefined
  return {index: slice.loadUint(16), hash: hexHash(slice.loadUintBig(256))}
}

/** Owns an environment-scoped, bounded archive cache; switching networks creates a new instance. */
export class ConfigVotingReader {
  private readonly archive = new Map<number, ConfigVotingSnapshot>()

  constructor(private readonly client: TonClient) {}

  /** Pins the current view to an indexed masterchain block from the selected network. */
  async current(signal: AbortSignal): Promise<ConfigVotingSnapshot> {
    const config = await this.client.getNetworkConfig()
    signal.throwIfAborted()
    if (!config.configAddress) {
      throw new Error("Network configuration has no config contract address")
    }

    const {blocks} = await this.client.getBlocks({workchain: -1, limit: 1, sort: "desc"})
    signal.throwIfAborted()
    if (!blocks[0]) throw new Error("Masterchain head is unavailable")

    return this.at(config.configAddress, blocks[0].seqno, signal, Number(blocks[0].gen_utime))
  }

  /** Pages successful proposal registrations from the selected network indexer. */
  async history(address: string, offset: number, signal: AbortSignal) {
    const limit = 10
    const {transactions} = await this.client.getTransactionsByOpcode(
      REGISTER_PROPOSAL,
      "in",
      limit,
      offset,
    )
    signal.throwIfAborted()

    return {
      proposals: configVoteRegistrations(transactions, address),
      nextOffset: offset + transactions.length,
      hasMore: transactions.length === limit,
    }
  }

  /** Resolves a direct link in this network, keeping the latest registration for a reused hash. */
  async proposal(hash: string, signal: AbortSignal, onProgress: (message: string) => void) {
    if (!/^[\da-f]{64}$/i.test(hash)) {
      throw new Error("Proposal hash must contain 64 hexadecimal characters")
    }

    signal.throwIfAborted()
    const normalizedHash = hash.toLowerCase()
    const started = performance.now()
    const snapshot = await this.current(signal)
    const context = {
      operation: "config_voting_proposal",
      target: snapshot.address,
      proposal: normalizedHash,
    }
    let outcome = "failed"
    let offset = 0

    try {
      let vote = snapshot.proposals.find(proposal => proposal.hash === normalizedHash)

      // Current storage is sufficient for active or expired entries. Removed proposals
      // need their newest successful registration before the archive result can be verified.
      while (!vote) {
        onProgress(`Searching proposal history · ${offset} transactions checked`)
        console.info("Finding proposal", {...context, offset, outcome: "searching"})

        const page = await this.history(snapshot.address, offset, signal)
        vote = page.proposals.find(proposal => proposal.hash === normalizedHash)
        offset = page.nextOffset
        if (!page.hasMore) break
      }

      if (!vote) {
        outcome = "not_found"
        throw new Error("Proposal was not found in this network")
      }

      outcome = "found"
      return {vote, snapshot}
    } finally {
      console.info("Finished proposal lookup", {
        ...context,
        duration_ms: performance.now() - started,
        transactions_checked: offset,
        outcome: signal.aborted ? "cancelled" : outcome,
      })
    }
  }

  /** Reads a historical account at a fixed block, preserving the rules and values used then. */
  async at(address: string, seqno: number, signal: AbortSignal, time?: number) {
    signal.throwIfAborted()
    const cached = this.archive.get(seqno)
    if (cached?.address === address) return cached

    const boc = await this.client.getShardAccountCell(address, seqno)
    signal.throwIfAborted()
    const account = loadShardAccount(Cell.fromBase64(boc).beginParse()).account
    if (account?.storage.state.type !== "active" || !account.storage.state.state.data) {
      throw new Error(`Configuration account is not active at block ${seqno}`)
    }

    const blockTime =
      time ?? (await this.client.getBlocks({workchain: -1, seqno, limit: 1})).blocks[0]?.gen_utime
    signal.throwIfAborted()
    if (blockTime === undefined) throw new Error(`Block ${seqno} timestamp is unavailable`)
    const result = parseConfigVotingState(
      account.storage.state.state.data,
      address,
      seqno,
      Number(blockTime),
    )

    const oldest = this.archive.keys().next().value
    if (this.archive.size >= 40 && oldest !== undefined) this.archive.delete(oldest)
    this.archive.set(seqno, result)

    return result
  }

  /** Locates the removal transition, then reports only outcomes supported by that state or a receipt. */
  async result(
    vote: ConfigVote,
    current: ConfigVotingSnapshot,
    signal: AbortSignal,
    onProgress: (message: string) => void,
  ): Promise<ConfigVoteResult> {
    const started = performance.now()
    const context = {
      operation: "config_voting_history",
      target: current.address,
      proposal: vote.hash,
    }
    console.info("Loading voting result", {...context, outcome: "started"})

    try {
      // Expiration is final even while lazy contract cleanup still retains the entry.
      if (vote.state && vote.state.expires <= current.time) {
        console.info("Loaded voting result", {
          ...context,
          duration_ms: performance.now() - started,
          outcome: "expired",
        })

        return {
          outcome: "expired",
          time: vote.state.expires,
          seqno: current.seqno,
          vote,
          snapshot: current,
          finalVoteRecorded: true,
          removed: false,
        }
      }

      const registrationSeqno = vote.registrationSeqno
      if (registrationSeqno === undefined) {
        throw new Error("Proposal registration block is unavailable")
      }

      onProgress("Checking voting confirmations")
      // Acceptance receipts are sparse and avoid a block-by-block archive scan.
      const {transactions} = await this.client.getTransactionsByOpcode(
        vote.critical ? ACCEPTED_CRITICAL : ACCEPTED_NORMAL,
        "out",
        100,
      )
      signal.throwIfAborted()

      const acceptance = transactions.find(
        transaction =>
          sameAddress(transaction.account, current.address) &&
          transaction.mc_block_seqno >= registrationSeqno &&
          transaction.mc_block_seqno <= current.seqno &&
          acceptedHash(transaction)?.hash === vote.hash,
      )

      let before: ConfigVotingSnapshot
      let after: ConfigVotingSnapshot

      if (acceptance) {
        onProgress("Loading the final voting round")
        before = await this.at(current.address, acceptance.mc_block_seqno - 1, signal)
        after = await this.at(current.address, acceptance.mc_block_seqno, signal, acceptance.now)
      } else {
        before = await this.at(current.address, registrationSeqno, signal, vote.registeredAt)
        if (!before.proposals.some(proposal => proposal.hash === vote.hash)) {
          throw new Error("Proposal is missing from its registration state")
        }

        after = current
        let checks = 0

        while (after.seqno - before.seqno > 1) {
          signal.throwIfAborted()
          onProgress(`Checking archived voting states · ${++checks}`)
          const middle = await this.at(
            current.address,
            Math.floor((before.seqno + after.seqno) / 2),
            signal,
          )
          if (middle.proposals.some(proposal => proposal.hash === vote.hash)) before = middle
          else after = middle

          if (checks % 5 === 0) {
            console.info("Reading voting archive", {
              ...context,
              checks,
              duration_ms: performance.now() - started,
              outcome: "running",
            })
          }
        }
      }

      const lastVote = before.proposals.find(proposal => proposal.hash === vote.hash)
      if (!lastVote?.state || after.proposals.some(proposal => proposal.hash === vote.hash)) {
        throw new Error("The archive does not contain a complete proposal transition")
      }

      const applied = sameParameter(
        lastVote.proposed,
        after.config.parameters.find(parameter => parameter.id === vote.parameterId),
      )
      const changed = !sameParameter(
        lastVote.proposed,
        before.config.parameters.find(parameter => parameter.id === vote.parameterId),
      )
      const rule = lastVote.critical ? before.rules.critical : before.rules.normal
      const currentSet = before.config.parameters.find(parameter => parameter.id === 34)
      const awaitingRollover =
        currentSet !== undefined &&
        cellFromParameter(currentSet).hash().toString("hex") !==
          hexHash(lastVote.state.validator_set_id)
      let outcome: ConfigVoteResult["outcome"] = "closed"

      if (acceptance) {
        outcome = applied ? "accepted" : "passed"
      } else if (applied && changed) {
        // External votes have no response message; the atomic config transition
        // still proves that this value was adopted as the proposal was removed
        outcome = "accepted"
      } else if (after.time >= lastVote.state.expires) {
        outcome = "expired"
      } else if (
        (awaitingRollover || !sameValidatorSet(before.config, after.config)) &&
        (lastVote.state.rounds_remaining === 0 ||
          (lastVote.state.remaining_weight >= 0n && lastVote.state.losses >= rule.max_losses))
      ) {
        outcome = "rejected"
      }

      const finalVote = acceptance && acceptedHash(acceptance)
      const tally = configVoteTally(lastVote, before)
      const validator =
        finalVote && tally?.set.validators.find(candidate => candidate.index === finalVote.index)

      // Only amend the final snapshot when the receipt and the same validator set
      // account for the threshold crossing; otherwise label it as the last snapshot.
      const finalVoteRecorded = Boolean(
        validator &&
          !lastVote.state.voters.has(validator.index) &&
          lastVote.state.remaining_weight >= 0n &&
          lastVote.state.remaining_weight - validator.weight < 0n &&
          sameValidatorSet(before.config, after.config),
      )
      let resultVote = lastVote

      // Keep the generated True leaf type and copy the dictionary before adding the last voter.
      if (finalVoteRecorded && validator) {
        const voters = Dictionary.empty<number, {kind: "True"}>(Dictionary.Keys.Uint(16))
        for (const [index, value] of lastVote.state.voters) voters.set(index, value)
        voters.set(validator.index, {kind: "True"})

        resultVote = {
          ...lastVote,
          state: {
            ...lastVote.state,
            voters,
            wins: lastVote.state.wins + 1,
            remaining_weight: lastVote.state.remaining_weight - validator.weight,
          },
        }
      }

      console.info("Loaded voting result", {
        ...context,
        duration_ms: performance.now() - started,
        outcome,
      })

      return {
        outcome,
        seqno: after.seqno,
        time: after.time,
        vote: resultVote,
        snapshot: before,
        finalVoteRecorded,
        removed: true,
      }
    } catch (error) {
      console.info("Voting result lookup ended", {
        ...context,
        duration_ms: performance.now() - started,
        outcome: signal.aborted ? "cancelled" : "error",
        error: error instanceof Error ? error.message : String(error),
      })
      throw error
    }
  }
}

function sameParameter(left?: NetworkConfigParameter, right?: NetworkConfigParameter) {
  return left === undefined || right === undefined
    ? left === right
    : cellFromParameter(left).equals(cellFromParameter(right))
}

function sameValidatorSet(before: NetworkConfig, after: NetworkConfig) {
  return sameParameter(
    before.parameters.find(parameter => parameter.id === 34),
    after.parameters.find(parameter => parameter.id === 34),
  )
}

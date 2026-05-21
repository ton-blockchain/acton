import {Buffer} from "node:buffer"

import type {TransactionInfo} from "@acton/shared-ui"
import {
  Address,
  Cell,
  Dictionary,
  loadStateInit,
  type AccountStatus,
  type Message,
  type StateInit,
  type Transaction,
  type TransactionActionPhase,
  type TransactionComputePhase,
  type TransactionDescription,
} from "@ton/core"

import {hashToHex} from "../components/utils"

import type {V3Transaction} from "./types"

export const buildTraceTransactionInfos = (
  transactionsMap: Record<string, V3Transaction>,
): TransactionInfo[] => {
  const txs = Object.values(transactionsMap)
  const txByLt = new Map(txs.map(tx => [tx.lt, tx]))
  const infoByLt = new Map<string, TransactionInfo>()

  const txInfos = txs.map(tx => {
    const info: TransactionInfo = {
      lt: tx.lt,
      address: parseTonAddress(tx.account),
      transaction: synthesizeTransaction(tx),
      vmLogDiff: "",
      executorLogs: "",
      executorActions: [],
      actions: undefined,
      outActions: [],
      contractName: undefined,
      shardAccountBefore: "",
      shardAccountAfter: "",
      parsedBody: undefined,
      parsedStorageBefore: undefined,
      parsedStorageAfter: undefined,
      parent: undefined,
      children: [],
    }
    infoByLt.set(tx.lt, info)
    return info
  })

  const parentByChildLt = new Map<string, string>()
  for (const tx of txs) {
    for (const childLt of tx.child_transactions) {
      parentByChildLt.set(childLt, tx.lt)
    }
  }

  for (const [lt, info] of infoByLt) {
    const tx = txByLt.get(lt)
    if (!tx) continue

    const parentLt = parentByChildLt.get(lt)
    if (parentLt) {
      info.parent = infoByLt.get(parentLt)
    }
    info.children = tx.child_transactions
      .map(childLt => infoByLt.get(childLt))
      .filter((child): child is TransactionInfo => child !== undefined)
  }

  return txInfos
}

const parseTonAddress = (address: string | undefined): Address | undefined => {
  if (!address) return undefined
  try {
    return Address.parse(address)
  } catch {
    return undefined
  }
}

const parseBigInt = (value: string | number | bigint | undefined, fallback = 0n): bigint => {
  if (value === undefined) return fallback
  try {
    return BigInt(value)
  } catch {
    return fallback
  }
}

const hashToBuffer = (hash: string | undefined): Buffer => {
  const hex = hash ? hashToHex(hash) : undefined
  return hex ? Buffer.from(hex, "hex") : Buffer.alloc(32)
}

const hashToBigInt = (hash: string | undefined): bigint => {
  const hex = hash ? hashToHex(hash) : undefined
  return hex ? BigInt(`0x${hex}`) : 0n
}

const accountAddressToBigInt = (address: string): bigint => {
  const parsed = parseTonAddress(address)
  return parsed ? BigInt(`0x${parsed.hash.toString("hex")}`) : 0n
}

const parseMessageCell = (body: string | undefined): Cell => {
  if (!body) return Cell.EMPTY
  try {
    return Cell.fromBase64(body)
  } catch {
    return Cell.EMPTY
  }
}

const parseStateInit = (body: string | undefined): StateInit | undefined => {
  if (!body) return undefined
  try {
    return loadStateInit(Cell.fromBase64(body).beginParse())
  } catch {
    return undefined
  }
}

const synthesizeMessage = (message: V3Transaction["in_msg"]): Message | undefined => {
  if (!message) return undefined

  const src = parseTonAddress(message.source)
  const dest = parseTonAddress(message.destination)
  const body = parseMessageCell(message.message_content?.body)
  const init = parseStateInit(message.init_state?.body)

  if (src && dest) {
    return {
      info: {
        type: "internal",
        ihrDisabled: message.ihr_disabled ?? true,
        bounce: message.bounce,
        bounced: message.bounced,
        src,
        dest,
        value: {coins: parseBigInt(message.value)},
        ihrFee: parseBigInt(message.ihr_fee),
        forwardFee: parseBigInt(message.fwd_fee),
        createdLt: parseBigInt(message.created_lt),
        createdAt: Number(message.created_at) || 0,
      },
      init,
      body,
    }
  }

  if (dest) {
    return {
      info: {
        type: "external-in",
        src: undefined,
        dest,
        importFee: parseBigInt(message.import_fee),
      },
      init,
      body,
    }
  }

  if (src) {
    return {
      info: {
        type: "external-out",
        src,
        dest: undefined,
        createdLt: parseBigInt(message.created_lt),
        createdAt: Number(message.created_at) || 0,
      },
      init,
      body,
    }
  }

  return undefined
}

const mapAccountStatus = (status: string | undefined): AccountStatus => {
  switch (status) {
    case "active": {
      return "active"
    }
    case "frozen": {
      return "frozen"
    }
    case "non-existing":
    case "nonexist": {
      return "non-existing"
    }
    default: {
      return "uninitialized"
    }
  }
}

const synthesizeComputePhase = (
  compute: V3Transaction["description"]["compute_ph"],
): TransactionComputePhase => {
  if (compute.skipped) {
    return {type: "skipped", reason: "no-state"}
  }

  return {
    type: "vm",
    success: compute.success,
    messageStateUsed: compute.msg_state_used ?? false,
    accountActivated: compute.account_activated ?? false,
    gasFees: parseBigInt(compute.gas_fees),
    gasUsed: parseBigInt(compute.gas_used),
    gasLimit: parseBigInt(compute.gas_limit),
    gasCredit: compute.gas_credit === undefined ? undefined : parseBigInt(compute.gas_credit),
    mode: compute.mode ?? 0,
    exitCode: compute.exit_code,
    exitArg: compute.exit_arg,
    vmSteps: compute.vm_steps ?? 0,
    vmInitStateHash: hashToBigInt(compute.vm_init_state_hash),
    vmFinalStateHash: hashToBigInt(compute.vm_final_state_hash),
  }
}

const synthesizeActionPhase = (tx: V3Transaction): TransactionActionPhase | undefined => {
  const action = tx.description.action
  if (!action) return undefined

  return {
    success: action.success,
    valid: action.valid ?? true,
    noFunds: action.no_funds ?? false,
    statusChange: "unchanged",
    totalFwdFees:
      action.total_fwd_fees === undefined ? undefined : parseBigInt(action.total_fwd_fees),
    totalActionFees:
      action.total_action_fees === undefined ? undefined : parseBigInt(action.total_action_fees),
    resultCode: action.result_code,
    resultArg: action.result_arg,
    totalActions: action.tot_actions ?? tx.out_msgs.length,
    specActions: action.spec_actions ?? 0,
    skippedActions: action.skipped_actions ?? 0,
    messagesCreated: action.msgs_created ?? tx.out_msgs.length,
    actionListHash: hashToBigInt(action.action_list_hash),
    totalMessageSize: {
      cells: parseBigInt(action.tot_msg_size?.cells),
      bits: parseBigInt(action.tot_msg_size?.bits),
    },
  }
}

const synthesizeDescription = (tx: V3Transaction): TransactionDescription => ({
  type: "generic",
  creditFirst: tx.description.credit_first ?? true,
  storagePhase: undefined,
  creditPhase: undefined,
  computePhase: synthesizeComputePhase(tx.description.compute_ph),
  actionPhase: synthesizeActionPhase(tx),
  bouncePhase: undefined,
  aborted: tx.description.aborted,
  destroyed: tx.description.destroyed ?? false,
})

const synthesizeTransaction = (tx: V3Transaction): Transaction => {
  const outMessages = Dictionary.empty<number, Message>(Dictionary.Keys.Uint(15))
  for (const [index, message] of tx.out_msgs.entries()) {
    const synthesized = synthesizeMessage(message)
    if (synthesized) {
      outMessages.set(index, synthesized)
    }
  }

  const hashBuffer = hashToBuffer(tx.hash)

  return {
    address: accountAddressToBigInt(tx.account),
    lt: parseBigInt(tx.lt),
    prevTransactionHash: hashToBigInt(tx.prev_trans_hash),
    prevTransactionLt: parseBigInt(tx.prev_trans_lt),
    now: tx.now,
    outMessagesCount: outMessages.size,
    oldStatus: mapAccountStatus(tx.orig_status),
    endStatus: mapAccountStatus(tx.end_status),
    inMessage: synthesizeMessage(tx.in_msg),
    outMessages,
    totalFees: {coins: parseBigInt(tx.total_fees)},
    stateUpdate: {
      oldHash: Buffer.alloc(32),
      newHash: Buffer.alloc(32),
    },
    description: synthesizeDescription(tx),
    raw: Cell.EMPTY,
    hash: () => hashBuffer,
  }
}

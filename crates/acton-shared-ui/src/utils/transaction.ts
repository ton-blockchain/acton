import {
  type Address,
  beginCell,
  Cell,
  loadOutList,
  loadTransaction,
  type OutAction,
  type Transaction,
} from "@ton/core"

import type {BackendTransaction, TransactionInfo} from "@/types"

const bigintToAddress = (addr: bigint | undefined): Address | undefined => {
  if (addr === undefined) return undefined
  try {
    const slice = beginCell().storeUint(4, 3).storeUint(0, 8).storeUint(addr, 256).asSlice()
    return slice.loadAddress()
  } catch {
    return undefined
  }
}

function parseActions(actionsBase64?: string): {
  outActions: OutAction[]
  actionsCell: Cell | undefined
} {
  if (!actionsBase64) return {outActions: [], actionsCell: undefined}
  try {
    const actionsCell = Cell.fromBase64(actionsBase64)
    const outActions = loadOutList(actionsCell.beginParse())
    return {outActions, actionsCell}
  } catch (error) {
    console.error("Failed to parse actions BOC", error)
    return {outActions: [], actionsCell: undefined}
  }
}

export function getTransactionOpcode(tx: Transaction): number | undefined {
  const inMessage = tx.inMessage
  if (!inMessage) return undefined

  const slice = inMessage.body.asSlice()
  if (slice.remainingBits < 32) return undefined

  let opcode = slice.loadUint(32)
  if (inMessage.info.type === "internal" && inMessage.info.bounced && slice.remainingBits >= 32) {
    opcode = slice.loadUint(32)
  }
  return opcode
}

export function processTransactions(transactions: BackendTransaction[]): TransactionInfo[] {
  const visited = new Map<string, TransactionInfo>()

  const txInfos = transactions.map(tx => {
    const parsedTx = loadTransaction(Cell.fromBase64(tx.raw_transaction).asSlice())
    const {outActions, actionsCell} = parseActions(tx.actions)

    const info: TransactionInfo = {
      lt: tx.lt,
      address: bigintToAddress(parsedTx.address),
      transaction: parsedTx,
      vmLogDiff: tx.vm_log_diff,
      executorLogs: tx.executor_logs,
      executorActions: tx.executor_actions ?? [],
      actions: actionsCell,
      outActions,
      contractName: tx.dest_contract_info,
      shardAccountBefore: tx.shard_account_before,
      shardAccountAfter: tx.shard_account,
      parsedBody: undefined,
      parsedStorageBefore: undefined,
      parsedStorageAfter: undefined,
      children: [],
      parent: undefined,
    }
    visited.set(tx.lt, info)
    return info
  })

  for (const tx of transactions) {
    const index = transactions.indexOf(tx)
    const info = txInfos[index]
    if (tx.parent_transaction && visited.has(tx.parent_transaction)) {
      info.parent = visited.get(tx.parent_transaction)
    }

    if (tx.child_transactions) {
      info.children = tx.child_transactions
        .map(childLt => visited.get(childLt))
        .filter((it): it is TransactionInfo => it !== undefined)
    }
  }

  return txInfos
}

export function computeSendMode(tx: TransactionInfo): number | undefined {
  const sender = tx.transaction.inMessage?.info.src
  if (!sender) return undefined

  const parent = tx.parent
  if (!parent) return undefined

  for (const action of parent.outActions) {
    if (
      action.type === "sendMsg" &&
      action.outMsg.info.dest?.toString() === tx.address?.toString()
    ) {
      return action.mode as number
    }
  }
  return undefined
}

export const RESERVE_MODE_CONSTANTS = {
  0: {
    name: "ReserveExact",
    description: "Reserves exactly the specified amount of nanoToncoin.",
  },
  1: {
    name: "ReserveAllExcept",
    description: "Reserves all but the specified amount of nanoToncoin.",
  },
  2: {
    name: "ReserveAtMost",
    description: "Reserves at most the specified amount of nanoToncoin.",
  },
  4: {
    name: "ReserveAddOriginalBalance",
    description:
      "Increases the amount by the original balance of the current account (before the compute phase), including all extra currencies.",
  },
  8: {
    name: "ReserveInvertSign",
    description: "Negates the amount value before performing the reservation.",
  },
  16: {
    name: "ReserveBounceIfActionFail",
    description: "Bounces the transaction if the reservation fails.",
  },
} as const

export interface ReserveModeInfo {
  readonly name: string
  readonly value: number
  readonly description: string
}

/**
 * Parse reserve mode number into an array of constants
 */
export function parseReserveMode(mode: number): ReserveModeInfo[] {
  const flags: ReserveModeInfo[] = []

  // Check base modes (mutually exclusive)
  if ((mode & 3) === 0) {
    flags.push({
      name: RESERVE_MODE_CONSTANTS[0].name,
      value: 0,
      description: RESERVE_MODE_CONSTANTS[0].description,
    })
  } else if ((mode & 3) === 1) {
    flags.push({
      name: RESERVE_MODE_CONSTANTS[1].name,
      value: 1,
      description: RESERVE_MODE_CONSTANTS[1].description,
    })
  } else if ((mode & 3) === 2) {
    flags.push({
      name: RESERVE_MODE_CONSTANTS[2].name,
      value: 2,
      description: RESERVE_MODE_CONSTANTS[2].description,
    })
  }

  // Check optional flags
  for (const [value, constant] of Object.entries(RESERVE_MODE_CONSTANTS)) {
    const flagValue = Number.parseInt(value, 10)
    if (flagValue >= 4 && mode & flagValue) {
      flags.push({
        name: constant.name,
        value: flagValue,
        description: constant.description,
      })
    }
  }

  return flags
}

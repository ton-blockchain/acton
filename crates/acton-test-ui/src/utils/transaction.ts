import {
  type Address,
  beginCell,
  Cell,
  loadOutList,
  loadTransaction,
  type OutAction,
  type Transaction,
} from "@ton/core"
import type { BackendTransaction } from "../types"
import type { TransactionInfo } from "../types/transaction"

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
  if (!actionsBase64) return { outActions: [], actionsCell: undefined }
  try {
    const actionsCell = Cell.fromBase64(actionsBase64)
    const outActions = loadOutList(actionsCell.beginParse()) as OutAction[]
    return { outActions, actionsCell }
  } catch (e) {
    console.error("Failed to parse actions BOC", e)
    return { outActions: [], actionsCell: undefined }
  }
}

export function getTransactionOpcode(tx: Transaction): number | undefined {
  const inMessage = tx.inMessage
  if (!inMessage) return undefined

  const slice = inMessage.body.asSlice()
  if (slice.remainingBits < 32) return undefined

  let opcode = slice.loadUint(32)
  if (inMessage.info.type === "internal" && inMessage.info.bounced) {
    if (slice.remainingBits >= 32) {
      opcode = slice.loadUint(32)
    }
  }
  return opcode
}

export function processTransactions(transactions: BackendTransaction[]): TransactionInfo[] {
  const visited = new Map<string, TransactionInfo>()

  const txInfos = transactions.map((tx) => {
    const parsedTx = loadTransaction(Cell.fromBase64(tx.raw_transaction).asSlice())
    const { outActions, actionsCell } = parseActions(tx.actions)

    const info: TransactionInfo = {
      lt: tx.lt,
      address: bigintToAddress(parsedTx.address),
      transaction: parsedTx,
      vmLogDiff: tx.vm_log_diff,
      executorLogs: tx.executor_logs,
      actions: actionsCell,
      outActions,
      contractName: tx.dest_contract_info,
      shardAccountBefore: tx.shard_account_before,
      shardAccountAfter: tx.shard_account,
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
        .map((childLt) => visited.get(childLt))
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

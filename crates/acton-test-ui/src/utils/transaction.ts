import {
  type Address,
  beginCell,
  Cell,
  loadOutList,
  loadTransaction,
  type OutAction,
  type Transaction,
} from "@ton/core"
import { Buffer } from "buffer"
import type { BackendTransaction } from "../types"
import type {
  ComputeInfo,
  ExternalTransactionInfoData,
  InternalTransactionInfoData,
  TransactionInfo,
  TransactionInfoData,
  TransactionMoney,
} from "../types/transaction"

export interface RawTransactionInfo {
  readonly transaction: string
  readonly parsedTransaction?: Transaction
  readonly fields: Record<string, unknown>
  readonly code?: string
  readonly sourceMap?: any
  readonly contractName?: string
  readonly parentId?: string
  readonly childrenIds: string[]
  readonly oldStorage?: string
  readonly newStorage?: string
  readonly callStack?: string
}

interface MutableTransactionInfo {
  readonly address: Address | undefined
  readonly transaction: Transaction
  readonly fields: Record<string, unknown>
  readonly opcode: number | undefined
  readonly computeInfo: ComputeInfo
  readonly money: TransactionMoney
  readonly amount: bigint | undefined
  readonly outActions: OutAction[]
  readonly c5: Cell | undefined
  readonly data: TransactionInfoData
  readonly code: Cell | undefined
  readonly sourceMap: any | undefined
  readonly contractName: string | undefined
  readonly oldStorage: Cell | undefined
  readonly newStorage: Cell | undefined
  readonly callStack: string | undefined
  parent: TransactionInfo | undefined
  children: TransactionInfo[]
}

const bigintToAddress = (addr: bigint | undefined): Address | undefined => {
  if (addr === undefined) return undefined
  try {
    const slice = beginCell().storeUint(4, 3).storeUint(0, 8).storeUint(addr, 256).asSlice()
    return slice.loadAddress()
  } catch {
    return undefined
  }
}

function txOpcode(transaction: Transaction): number | undefined {
  const inMessage = transaction.inMessage
  const isBounced = inMessage?.info.type === "internal" ? inMessage.info.bounced : false
  let opcode: number | undefined
  const slice = inMessage?.body.asSlice()
  if (slice) {
    if (isBounced && slice.remainingBits >= 32) {
      slice.loadUint(32)
    }
    if (slice.remainingBits >= 32) {
      opcode = slice.loadUint(32)
    }
  }
  return opcode
}

function txData(transaction: Transaction): TransactionInfoData {
  const inMessage = transaction.inMessage
  if (inMessage?.info.type === "internal") {
    return {} satisfies InternalTransactionInfoData
  }
  return {} satisfies ExternalTransactionInfoData
}

export const findFinalActions = (
  logs: string | undefined,
  actionsHex?: string,
): { outActions: OutAction[]; c5: undefined | Cell } => {
  let outActions: OutAction[] = []
  let c5: Cell | undefined

  if (actionsHex) {
    try {
      const actionsCell = Cell.fromBoc(Buffer.from(actionsHex, "hex"))[0]
      outActions = loadOutList(actionsCell.beginParse()) as OutAction[]
      c5 = actionsCell
      return { outActions, c5 }
    } catch (e) {
      console.error("Failed to parse actions BOC", e)
    }
  }

  if (!logs) return { outActions, c5 }
  for (const line of logs.split("\n")) {
    if (line.startsWith("final c5:")) {
      try {
        const cellBoc = Buffer.from(line.slice("final c5: C{".length, -1), "hex")
        const c5Cell = Cell.fromBoc(cellBoc)[0]
        const slice = c5Cell.beginParse()
        outActions = loadOutList(slice) as OutAction[]
        c5 = c5Cell
      } catch (e) {
        console.error("Failed to parse c5", e)
      }
    }
  }
  return { outActions, c5 }
}

const calculateSentTotal = (tx: Transaction): bigint => {
  let total = 0n
  for (const msg of tx.outMessages.values()) {
    if (msg.info.type === "internal") {
      total += msg.info.value.coins
    }
  }
  return total
}

const computeFinalData = (
  transaction: Transaction,
): {
  money: TransactionMoney
  amount: bigint | undefined
  computeInfo: ComputeInfo
} => {
  if (!transaction.inMessage) {
    throw new Error("No in_message was found in result tx")
  }
  const amount =
    transaction.inMessage.info.type === "internal"
      ? transaction.inMessage.info.value.coins
      : undefined
  const sentTotal = calculateSentTotal(transaction)
  const totalFees = transaction.totalFees.coins
  if (transaction.description.type !== "generic") {
    throw new Error("Only generic transactions are supported")
  }
  const computePhase = transaction.description.computePhase
  const computeInfo: ComputeInfo =
    computePhase.type === "skipped"
      ? "skipped"
      : {
          success: computePhase.success,
          exitCode:
            computePhase.exitCode === 0
              ? (transaction.description.actionPhase?.resultCode ?? 0)
              : computePhase.exitCode,
          vmSteps: computePhase.vmSteps,
          gasUsed: computePhase.gasUsed,
          gasFees: computePhase.gasFees,
        }
  const forwardFee =
    transaction.inMessage.info.type === "internal" ? transaction.inMessage.info.forwardFee : 0n
  const money: TransactionMoney = { sentTotal, totalFees, forwardFee }
  return { money, amount, computeInfo }
}

const processRawTx = (
  tx: RawTransactionInfo,
  txs: RawTransactionInfo[],
  visited: Map<string, TransactionInfo>,
): TransactionInfo => {
  const parsedTx =
    tx.parsedTransaction ?? loadTransaction(Cell.fromBase64(tx.transaction).asSlice())
  const lt = parsedTx.lt.toString()
  const cached = visited.get(lt)
  if (cached) return cached

  const address = bigintToAddress(parsedTx.address)
  const { computeInfo, amount, money } = computeFinalData(parsedTx)
  const { outActions, c5 } = findFinalActions(
    tx.fields.vmLogs as string,
    tx.fields.actions as string | undefined,
  )

  const result: MutableTransactionInfo = {
    address,
    transaction: parsedTx,
    fields: tx.fields,
    parent: undefined,
    opcode: txOpcode(parsedTx),
    computeInfo,
    money,
    amount,
    outActions,
    c5,
    data: txData(parsedTx),
    code: tx.code ? Cell.fromHex(tx.code) : undefined,
    sourceMap: tx.sourceMap,
    contractName: tx.contractName,
    children: [],
    oldStorage: tx.oldStorage ? Cell.fromHex(tx.oldStorage) : undefined,
    newStorage: tx.newStorage ? Cell.fromHex(tx.newStorage) : undefined,
    callStack: tx.callStack,
  }
  visited.set(lt, result as TransactionInfo)

  const parent = txs.find((it) => it.parsedTransaction?.lt?.toString() === tx.parentId)
  result.parent = parent ? processRawTx(parent, txs, visited) : undefined
  result.children = tx.childrenIds
    .map((childId) => txs.find((it) => it.parsedTransaction?.lt?.toString() === childId))
    .filter((it): it is RawTransactionInfo => it !== undefined)
    .map((tx) => processRawTx(tx, txs, visited))

  return result as TransactionInfo
}

export function processTransactions(transactions: BackendTransaction[]): TransactionInfo[] {
  const rawTxs: RawTransactionInfo[] = transactions.map((tx) => ({
    transaction: tx.raw_transaction,
    fields: {
      vmLogs: tx.vm_log_diff,
      executorLogs: tx.executor_logs,
      actions: tx.actions,
    },
    contractName: tx.dest_contract_info,
    parentId: tx.parent_transaction !== null ? tx.parent_transaction : undefined,
    childrenIds: tx.child_transactions,
  }))

  const txs: RawTransactionInfo[] = rawTxs.map((tx) => {
    return {
      ...tx,
      parsedTransaction: loadTransaction(Cell.fromBase64(tx.transaction).asSlice()),
    } satisfies RawTransactionInfo
  })

  const visited = new Map<string, TransactionInfo>()
  return txs.map((tx) => processRawTx(tx, txs, visited))
}

export function computeSendMode(
  tx: TransactionInfo,
  transactions: TransactionInfo[],
): number | undefined {
  const sender = tx.transaction.inMessage?.info.src
  if (!sender) return undefined
  const txsToSender = transactions.filter(
    (it) => it.transaction.inMessage?.info.dest?.toString() === sender.toString(),
  )
  for (const txToSender of txsToSender) {
    for (const action of txToSender.outActions) {
      if (
        action.type === "sendMsg" &&
        action.outMsg.info.dest?.toString() === tx.address?.toString()
      ) {
        return action.mode as number
      }
    }
  }
  return undefined
}

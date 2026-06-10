import {
  type Address,
  beginCell,
  Cell,
  loadOutList,
  loadTransaction,
  type OutAction,
  type Transaction,
} from "@ton/core"

import type {BackendContractInfo, BackendTransaction, TransactionInfo} from "@/types"
import type {ContractData, ValueFlowItem} from "@/types/transaction"
import {getMessageOpcode, getShardAccountBalance, resolveAbiOpcodeName} from "@/utils/messageBody"

interface ValueFlowAccumulator extends ValueFlowItem {
  readonly before: bigint
  readonly after: bigint
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
  return inMessage ? getMessageOpcode(inMessage) : undefined
}

export function resolveTransactionOpcodeName(
  tx: TransactionInfo,
  contracts: Map<string, ContractData>,
  allContracts: readonly BackendContractInfo[],
): string | undefined {
  const opcode = getTransactionOpcode(tx.transaction)
  if (opcode === undefined) {
    return undefined
  }
  if (opcode === 0) {
    return "Text Comment"
  }

  const inMessage = tx.transaction.inMessage
  const targetContract = tx.address ? contracts.get(tx.address.toString()) : undefined
  const destinationContract = inMessage?.info.dest
    ? contracts.get(inMessage.info.dest.toString())
    : targetContract
  const sourceContract = inMessage?.info.src
    ? contracts.get(inMessage.info.src.toString())
    : undefined
  const isBouncedInternal = inMessage?.info.type === "internal" && inMessage.info.bounced

  if (isBouncedInternal) {
    return (
      resolveAbiOpcodeName(targetContract?.abi, opcode, "outgoing") ??
      resolveAbiOpcodeName(sourceContract?.abi, opcode, "incoming") ??
      findOpcodeNameInContracts(opcode, allContracts)
    )
  }

  return (
    resolveAbiOpcodeName(destinationContract?.abi, opcode, "incoming") ??
    resolveAbiOpcodeName(sourceContract?.abi, opcode, "outgoing") ??
    resolveAbiOpcodeName(targetContract?.abi, opcode) ??
    findOpcodeNameInContracts(opcode, allContracts)
  )
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
      contractAbi: undefined,
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

export function buildValueFlowItems(transactions: readonly TransactionInfo[]): ValueFlowItem[] {
  const flowByAddress = new Map<string, ValueFlowAccumulator>()

  for (const tx of [...transactions].sort(compareTransactionInfoByLt)) {
    const address = tx.address?.toString()
    if (!address) {
      continue
    }

    const before = tx.accountBalanceBefore ?? getShardAccountBalance(tx.shardAccountBefore)
    const after = tx.accountBalanceAfter ?? getShardAccountBalance(tx.shardAccountAfter)
    if (before === undefined || after === undefined) {
      continue
    }

    const previous = flowByAddress.get(address)
    const initialBefore = previous?.before ?? before

    flowByAddress.set(address, {
      address,
      before: initialBefore,
      after,
      change: after - initialBefore,
      fee: (previous?.fee ?? 0n) + tx.transaction.totalFees.coins,
    })
  }

  return [...flowByAddress.values()]
    .map(({address, change, fee}) => ({address, change, fee}))
    .sort((left, right) => left.address.localeCompare(right.address))
}

function compareTransactionInfoByLt(left: TransactionInfo, right: TransactionInfo): number {
  const leftLt = parseBigInt(left.lt)
  const rightLt = parseBigInt(right.lt)
  if (leftLt === rightLt) {
    return 0
  }
  return leftLt < rightLt ? -1 : 1
}

function parseBigInt(value: string | undefined): bigint {
  try {
    return value === undefined ? 0n : BigInt(value)
  } catch {
    return 0n
  }
}

function findOpcodeNameInContracts(
  opcode: number,
  allContracts: readonly BackendContractInfo[],
): string | undefined {
  for (const contract of allContracts) {
    const name = resolveAbiOpcodeName(contract.abi, opcode)
    if (name) {
      return name
    }
  }
  return undefined
}

export function getTransactionComputePhase(tx: Transaction) {
  const description = tx.description
  if (description.type === "generic" || description.type === "tick-tock") {
    return description.computePhase
  }
  return
}

export function getTransactionActionPhase(tx: Transaction) {
  const description = tx.description
  if (description.type === "generic" || description.type === "tick-tock") {
    return description.actionPhase
  }
  return
}

export function getTransactionTriggerLabel(tx: Transaction): string | undefined {
  const description = tx.description
  if (description.type === "tick-tock") {
    return description.isTock ? "Tock" : "Tick"
  }
  return undefined
}

export function getTransactionSourceLabel(tx: Transaction): string | undefined {
  const inMessage = tx.inMessage
  if (inMessage?.info.type === "external-in") {
    return "External In"
  }
  return getTransactionTriggerLabel(tx)
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
    description: "Reserves exactly the specified amount of nanograms.",
  },
  1: {
    name: "ReserveAllExcept",
    description: "Reserves all but the specified amount of nanograms.",
  },
  2: {
    name: "ReserveAtMost",
    description: "Reserves at most the specified amount of nanograms.",
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

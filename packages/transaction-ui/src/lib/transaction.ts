import {
  Address,
  beginCell,
  Cell,
  loadOutList,
  loadTransaction,
  type Message,
  type OutAction,
  type Transaction,
} from "@ton/core"

import type {BackendContractInfo, BackendTransaction} from "../model/backend"
import type {
  ContractData,
  TransactionInfo,
  ValueFlowAsset,
  ValueFlowAssetChange,
  ValueFlowAssetMovement,
  ValueFlowItem,
} from "../model/transaction"
import {getMessageOpcode, getShardAccountBalance, resolveAbiOpcodeName} from "./messageBody"

interface ValueFlowAccumulator {
  readonly address: string
  readonly before?: bigint
  readonly after?: bigint
  readonly fee: bigint
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
      id: parsedTx.hash().toString("hex"),
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

export function buildValueFlowItems(
  transactions: readonly TransactionInfo[],
  assetMovements: readonly ValueFlowAssetMovement[] = [],
): ValueFlowItem[] {
  const flowByAddress = new Map<string, ValueFlowAccumulator>()
  const assetChangesByAddress = new Map<string, Map<string, ValueFlowAssetChange>>()

  for (const tx of [...transactions].sort(compareTransactionInfoByLt)) {
    const address = tx.address?.toString()
    if (!address) {
      continue
    }

    const previous = flowByAddress.get(address)
    const before = getTransactionBalanceBefore(tx)
    const after = getTransactionBalanceAfter(tx)

    flowByAddress.set(address, {
      address,
      before: previous?.before ?? before,
      after: after ?? previous?.after,
      fee: (previous?.fee ?? 0n) + tx.transaction.totalFees.coins,
    })
  }

  for (const movement of assetMovements) {
    if (movement.amount <= 0n) {
      continue
    }

    const source = normalizeValueFlowAddress(movement.source)
    const destination = normalizeValueFlowAddress(movement.destination)
    if (source) {
      addAssetChange(assetChangesByAddress, source, movement.asset, -movement.amount)
    }
    if (destination) {
      addAssetChange(assetChangesByAddress, destination, movement.asset, movement.amount)
    }
  }

  const addresses = new Set([...flowByAddress.keys(), ...assetChangesByAddress.keys()])
  return [...addresses]
    .flatMap(address => {
      const flow = flowByAddress.get(address)
      const hasTonBalance = flow?.before !== undefined && flow.after !== undefined
      const assetChanges = [...(assetChangesByAddress.get(address)?.values() ?? [])].filter(
        change => change.change !== 0n,
      )
      if (!hasTonBalance && assetChanges.length === 0) {
        return []
      }

      return [
        {
          address,
          change: hasTonBalance ? flow.after - flow.before : 0n,
          fee: flow?.fee ?? 0n,
          assetChanges,
        },
      ]
    })
    .sort((left, right) => left.address.localeCompare(right.address))
}

function addAssetChange(
  changesByAddress: Map<string, Map<string, ValueFlowAssetChange>>,
  address: string,
  asset: ValueFlowAsset,
  change: bigint,
): void {
  const changes = changesByAddress.get(address) ?? new Map<string, ValueFlowAssetChange>()
  const previous = changes.get(asset.id)
  changes.set(asset.id, {
    asset: {
      id: asset.id,
      symbol: previous?.asset.symbol ?? asset.symbol,
      decimals: previous?.asset.decimals ?? asset.decimals,
    },
    change: (previous?.change ?? 0n) + change,
  })
  changesByAddress.set(address, changes)
}

function normalizeValueFlowAddress(address: string | undefined): string | undefined {
  if (!address) {
    return undefined
  }

  try {
    return Address.parse(address).toString()
  } catch {
    return undefined
  }
}

function getTransactionBalanceBefore(tx: TransactionInfo): bigint | undefined {
  return (
    tx.accountBalanceBefore ??
    getShardAccountBalance(tx.shardAccountBefore) ??
    (tx.transaction.oldStatus === "non-existing" ? 0n : undefined)
  )
}

function getTransactionBalanceAfter(tx: TransactionInfo): bigint | undefined {
  return (
    tx.accountBalanceAfter ??
    getShardAccountBalance(tx.shardAccountAfter) ??
    (tx.transaction.endStatus === "non-existing" ? 0n : undefined)
  )
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

type SendMessageAction = Extract<OutAction, {type: "sendMsg"}>

const optionalAddressEquals = (
  left: Address | null | undefined,
  right: Address | null | undefined,
): boolean => left?.toString() === right?.toString()

const cellHashEquals = (left: Cell, right: Cell): boolean =>
  left.hash().toString("hex") === right.hash().toString("hex")

const internalMessagesEqual = (left: Message, right: Message): boolean => {
  if (left.info.type !== "internal" || right.info.type !== "internal") {
    return false
  }

  return (
    optionalAddressEquals(left.info.src, right.info.src) &&
    optionalAddressEquals(left.info.dest, right.info.dest) &&
    left.info.value.coins === right.info.value.coins &&
    left.info.bounce === right.info.bounce &&
    left.info.bounced === right.info.bounced &&
    left.info.createdLt === right.info.createdLt &&
    left.info.createdAt === right.info.createdAt &&
    cellHashEquals(left.body, right.body)
  )
}

const relaxedInternalMessageMatchesChild = (
  actionMessage: SendMessageAction["outMsg"],
  childMessage: Message,
): boolean => {
  if (actionMessage.info.type !== "internal" || childMessage.info.type !== "internal") {
    return false
  }

  if (
    actionMessage.info.src &&
    !optionalAddressEquals(actionMessage.info.src, childMessage.info.src)
  ) {
    return false
  }

  if (
    actionMessage.info.createdLt !== 0n &&
    actionMessage.info.createdLt !== childMessage.info.createdLt
  ) {
    return false
  }

  if (
    actionMessage.info.createdAt !== 0 &&
    actionMessage.info.createdAt !== childMessage.info.createdAt
  ) {
    return false
  }

  return (
    optionalAddressEquals(actionMessage.info.dest, childMessage.info.dest) &&
    actionMessage.info.value.coins === childMessage.info.value.coins &&
    actionMessage.info.bounce === childMessage.info.bounce &&
    actionMessage.info.bounced === childMessage.info.bounced &&
    cellHashEquals(actionMessage.body, childMessage.body)
  )
}

export function computeSendMode(tx: TransactionInfo): number | undefined {
  const inMessage = tx.transaction.inMessage
  if (inMessage?.info.type !== "internal") return undefined

  const parent = tx.parent
  if (!parent) return undefined

  const parentInternalOutMessages = [...parent.transaction.outMessages.values()].filter(
    message => message.info.type === "internal",
  )
  const parentOutMessageIndex = parentInternalOutMessages.findIndex(message =>
    internalMessagesEqual(message, inMessage),
  )
  if (parentOutMessageIndex === -1) {
    return undefined
  }

  let internalSendMessageIndex = 0
  for (const action of parent.outActions) {
    if (action.type !== "sendMsg" || action.outMsg.info.type !== "internal") {
      continue
    }

    const actionIndex = internalSendMessageIndex
    internalSendMessageIndex += 1

    if (
      actionIndex === parentOutMessageIndex &&
      relaxedInternalMessageMatchesChild(action.outMsg, inMessage)
    ) {
      return action.mode as number
    }
  }
  return undefined
}

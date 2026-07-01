import {
  beginCell,
  Cell,
  loadShardAccount,
  type Message,
  type MessageRelaxed,
  type OutAction,
  type StateInit,
  storeMessage,
  storeMessageRelaxed,
  storeOutAction,
  storeOutList,
  storeStateInit,
} from "@ton/core"

export function formatCellBocHex(cell: Cell): string {
  return cell.toBoc({idx: false, crc32: false}).toString("hex")
}

export function formatShardAccountDataBocHex(shardAccountBase64: string): string | undefined {
  try {
    const shard = loadShardAccount(Cell.fromBase64(shardAccountBase64).beginParse())
    const state = shard.account?.storage.state
    if (state?.type !== "active" || !state.state.data) {
      return undefined
    }

    return formatCellBocHex(state.state.data)
  } catch {
    return undefined
  }
}

export function formatMessageBocHex(message: Message): string {
  return formatCellBocHex(beginCell().store(storeMessage(message)).endCell())
}

export function formatMessageRelaxedBocHex(message: MessageRelaxed): string {
  return formatCellBocHex(beginCell().store(storeMessageRelaxed(message)).endCell())
}

export function formatStateInitBocHex(stateInit: StateInit): string {
  return formatCellBocHex(beginCell().store(storeStateInit(stateInit)).endCell())
}

export function formatOutActionBocHex(action: OutAction): string {
  return formatCellBocHex(beginCell().store(storeOutAction(action)).endCell())
}

export function formatOutListBocHex(actions: readonly OutAction[]): string {
  return formatCellBocHex(
    beginCell()
      .store(storeOutList([...actions]))
      .endCell(),
  )
}

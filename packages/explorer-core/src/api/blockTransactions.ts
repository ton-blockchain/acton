import type {TonClient} from "./client"
import type {
  V2BlockTransactionListItem,
  V3AddressBookRow,
  V3Block,
  V3TransactionListItem,
} from "./types"
import {parseAddress} from "../components/utils"

export type BlockTransactionListItem = V2BlockTransactionListItem | V3TransactionListItem

type BlockTransactionsClient = Pick<TonClient, "getBlockTransactions" | "getBlockTransactionsV2">

type BlockTransactionsBlock = Pick<
  V3Block,
  "workchain" | "shard" | "seqno" | "root_hash" | "file_hash" | "tx_count"
>

type BlockTransactionsCursorValue =
  | {
      readonly source: "v3"
      readonly offset: number
    }
  | {
      readonly source: "v2"
      readonly afterLt: string
      readonly afterHash: string
    }

const cursorValue = Symbol("blockTransactionsCursorValue")

export interface BlockTransactionsCursor {
  readonly [cursorValue]: BlockTransactionsCursorValue
}

export interface BlockTransactionsPage {
  readonly transactions: readonly BlockTransactionListItem[]
  readonly addressBook: Readonly<Record<string, V3AddressBookRow>>
  readonly nextCursor?: BlockTransactionsCursor
  readonly unavailable: boolean
}

export async function loadBlockTransactionsPage(
  client: BlockTransactionsClient,
  block: BlockTransactionsBlock,
  limit: number,
  cursor?: BlockTransactionsCursor,
): Promise<BlockTransactionsPage> {
  const continuation = cursor?.[cursorValue]
  if (continuation?.source === "v2") {
    const response = await client.getBlockTransactionsV2({
      workchain: block.workchain,
      shard: block.shard,
      seqno: block.seqno,
      rootHash: block.root_hash,
      fileHash: block.file_hash,
      count: limit,
      afterLt: continuation.afterLt,
      afterHash: continuation.afterHash,
    })
    return v2Page(response.transactions, response.incomplete)
  }

  const offset = continuation?.source === "v3" ? continuation.offset : 0
  let v3Response: Awaited<ReturnType<BlockTransactionsClient["getBlockTransactions"]>> | undefined
  try {
    v3Response = await client.getBlockTransactions({
      workchain: block.workchain,
      shard: block.shard,
      seqno: block.seqno,
      limit,
      offset,
    })
  } catch (error) {
    if (continuation) {
      throw error
    }
  }

  const v3Transactions = v3Response?.transactions ?? []
  if (v3Transactions.length > 0 || block.tx_count === 0 || continuation) {
    const nextOffset = offset + v3Transactions.length
    return {
      transactions: v3Transactions,
      addressBook: v3Response?.address_book ?? {},
      nextCursor:
        v3Transactions.length === limit && nextOffset < block.tx_count
          ? makeCursor({source: "v3", offset: nextOffset})
          : undefined,
      unavailable: false,
    }
  }

  try {
    const fallback = await client.getBlockTransactionsV2({
      workchain: block.workchain,
      shard: block.shard,
      seqno: block.seqno,
      rootHash: block.root_hash,
      fileHash: block.file_hash,
      count: limit,
    })
    return {
      ...v2Page(fallback.transactions, fallback.incomplete),
      addressBook: v3Response?.address_book ?? {},
      unavailable: fallback.transactions.length === 0,
    }
  } catch {
    return {
      transactions: [],
      addressBook: v3Response?.address_book ?? {},
      unavailable: true,
    }
  }
}

function v2Page(
  transactions: readonly V2BlockTransactionListItem[],
  incomplete: boolean,
): BlockTransactionsPage {
  const lastTransaction = transactions.at(-1)
  const afterHash = lastTransaction
    ? parseAddress(lastTransaction.account)?.hash.toString("hex")
    : undefined
  return {
    transactions,
    addressBook: {},
    nextCursor:
      incomplete && lastTransaction && afterHash
        ? makeCursor({source: "v2", afterLt: lastTransaction.lt, afterHash})
        : undefined,
    unavailable: false,
  }
}

function makeCursor(value: BlockTransactionsCursorValue): BlockTransactionsCursor {
  return {[cursorValue]: value}
}

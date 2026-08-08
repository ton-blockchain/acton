import {describe, expect, test} from "bun:test"

import {loadBlockTransactionsPage} from "../src/api/blockTransactions"
import type {TonClient} from "../src/api/client"
import type {V2BlockTransactionListItem, V3TransactionListItem} from "../src/api/types"

const block = {
  workchain: 0,
  shard: "4000000000000000",
  seqno: 42,
  root_hash: "root",
  file_hash: "file",
  tx_count: 2,
}

describe("block transaction fallback", () => {
  test("keeps V3 pagination behind an opaque cursor", async () => {
    const requests: unknown[] = []
    const transactions = [v3Transaction("1"), v3Transaction("2")]
    const client = blockTransactionsClient({
      getBlockTransactions: options => {
        requests.push(options)
        const offset = options.offset ?? 0
        return Promise.resolve({transactions: [transactions[offset]], address_book: {}})
      },
      getBlockTransactionsV2: () => {
        throw new Error("V2 must not be called")
      },
    })

    const firstPage = await loadBlockTransactionsPage(client, block, 1)
    const secondPage = await loadBlockTransactionsPage(client, block, 1, firstPage.nextCursor)

    expect(firstPage.transactions).toEqual([transactions[0]])
    expect(secondPage.transactions).toEqual([transactions[1]])
    expect(secondPage.nextCursor).toBeUndefined()
    expect(requests).toEqual([
      {workchain: 0, shard: "4000000000000000", seqno: 42, limit: 1, offset: 0},
      {workchain: 0, shard: "4000000000000000", seqno: 42, limit: 1, offset: 1},
    ])
  })

  test("falls back to V2 and keeps its pagination details out of the caller", async () => {
    const fallbackRequests: unknown[] = []
    const transactions = [v2Transaction("1", "a"), v2Transaction("2", "b")]
    const client = blockTransactionsClient({
      getBlockTransactions: () => Promise.resolve({transactions: [], address_book: {}}),
      getBlockTransactionsV2: options => {
        fallbackRequests.push(options)
        const transaction = options.afterLt ? transactions[1] : transactions[0]
        return Promise.resolve(v2Response([transaction], options.afterLt === undefined))
      },
    })

    const firstPage = await loadBlockTransactionsPage(client, block, 1)
    const secondPage = await loadBlockTransactionsPage(client, block, 1, firstPage.nextCursor)

    expect(firstPage.transactions).toEqual([transactions[0]])
    expect(firstPage.unavailable).toBeFalse()
    expect(secondPage.transactions).toEqual([transactions[1]])
    expect(fallbackRequests).toEqual([
      {
        workchain: 0,
        shard: "4000000000000000",
        seqno: 42,
        rootHash: "root",
        fileHash: "file",
        count: 1,
      },
      {
        workchain: 0,
        shard: "4000000000000000",
        seqno: 42,
        rootHash: "root",
        fileHash: "file",
        count: 1,
        afterLt: "1",
        afterHash: "a".repeat(64),
      },
    ])
  })

  test("reports unavailable when neither API returns block transactions", async () => {
    const client = blockTransactionsClient({
      getBlockTransactions: () => Promise.reject(new Error("V3 unavailable")),
      getBlockTransactionsV2: () => Promise.reject(new Error("V2 unavailable")),
    })

    const page = await loadBlockTransactionsPage(client, block, 100)

    expect(page.transactions).toEqual([])
    expect(page.nextCursor).toBeUndefined()
    expect(page.unavailable).toBeTrue()
  })
})

function blockTransactionsClient(
  client: Pick<TonClient, "getBlockTransactions" | "getBlockTransactionsV2">,
): Pick<TonClient, "getBlockTransactions" | "getBlockTransactionsV2"> {
  return client
}

function v3Transaction(lt: string): V3TransactionListItem {
  return {
    account: `0:${"c".repeat(64)}`,
    lt,
    hash: lt.padStart(64, "0"),
    description: {},
  } as V3TransactionListItem
}

function v2Transaction(lt: string, accountHashCharacter: string): V2BlockTransactionListItem {
  return {
    "@type": "blocks.shortTxId",
    mode: 0,
    account: `0:${accountHashCharacter.repeat(64)}`,
    lt,
    hash: lt.padStart(64, "0"),
  }
}

function v2Response(
  transactions: readonly V2BlockTransactionListItem[],
  incomplete: boolean,
): Awaited<ReturnType<TonClient["getBlockTransactionsV2"]>> {
  return {
    "@type": "blocks.transactions",
    id: {
      "@type": "ton.blockIdExt",
      workchain: block.workchain,
      shard: block.shard,
      seqno: block.seqno,
      root_hash: block.root_hash,
      file_hash: block.file_hash,
    },
    req_count: transactions.length,
    incomplete,
    transactions,
  }
}

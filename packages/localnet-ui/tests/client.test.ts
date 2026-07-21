import {expect, mock, test} from "bun:test"
import {beginCell} from "@ton/core"

import {TonClient} from "../src/explorer/api/client"

test("raw blocks are loaded from the selected TonAPI LiteServer", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  const blockCell = beginCell().storeUint(0x11_ef_55_aa, 32).endCell()
  globalThis.fetch = mock(async input => {
    requests.push(new URL(input.toString()))
    return Response.json({data: blockCell.toBoc().toString("hex")})
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })
    const extendedBlockId =
      "(-1,8000000000000000,81088003,aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)"

    const result = await client.getRawBlockBoc(extendedBlockId, "testnet")

    expect(result.hash().equals(blockCell.hash())).toBe(true)
    expect(requests).toHaveLength(1)
    expect(requests[0]?.origin).toBe("https://testnet.tonapi.io")
    expect(decodeURIComponent(requests[0]?.pathname.split("/").at(-1) ?? "")).toBe(extendedBlockId)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("getShardAccountCell reads the unwrapped V2 response", async () => {
  const originalFetch = globalThis.fetch
  const requests: string[] = []
  globalThis.fetch = mock(async input => {
    requests.push(input.toString())
    return Response.json({
      ok: true,
      result: {
        "@type": "tvm.cell",
        bytes: "te6cckEBAQEAAgAAAA==",
      },
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await expect(client.getShardAccountCell("EQAddress", 123)).resolves.toBe("te6cckEBAQEAAgAAAA==")
    expect(requests).toEqual([
      "https://toncenter.example/api/v2/getShardAccountCell?address=EQAddress&seqno=123",
    ])
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("masterchain shard blocks are resolved from the V2 shard snapshot", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  const shardBlock = {
    workchain: 0,
    shard: "8000000000000000",
    seqno: 84_021_699,
    root_hash: "root-hash",
    file_hash: "file-hash",
    created_by: "created-by",
    rand_seed: "rand-seed",
    start_lt: "89777846000000",
    end_lt: "89777846000001",
    gen_utime: "1783903686",
    tx_count: 0,
  }
  globalThis.fetch = mock(async input => {
    const url = new URL(input.toString())
    requests.push(url)
    if (url.pathname.endsWith("/getShards")) {
      return Response.json({
        ok: true,
        result: {
          "@type": "blocks.shards",
          shards: [
            {
              "@type": "ton.blockIdExt",
              workchain: 0,
              shard: "-9223372036854775808",
              seqno: shardBlock.seqno,
              root_hash: shardBlock.root_hash,
              file_hash: shardBlock.file_hash,
            },
          ],
        },
      })
    }
    return Response.json({blocks: [shardBlock]})
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await expect(client.getMasterchainBlockShards(79_299_165)).resolves.toEqual({
      blocks: [shardBlock],
    })
    expect(requests).toHaveLength(2)
    expect(requests[0]?.pathname).toBe("/api/v2/getShards")
    expect(requests[0]?.searchParams.get("seqno")).toBe("79299165")
    expect(requests[1]?.pathname).toBe("/api/v3/blocks")
    expect(requests[1]?.searchParams.get("workchain")).toBe("0")
    expect(requests[1]?.searchParams.get("shard")).toBe("8000000000000000")
    expect(requests[1]?.searchParams.get("seqno")).toBe("84021699")
    expect(requests[1]?.searchParams.get("root_hash")).toBe(shardBlock.root_hash)
    expect(requests[1]?.searchParams.get("file_hash")).toBe(shardBlock.file_hash)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("account history requests forward the requested sort order", async () => {
  const originalFetch = globalThis.fetch
  const requests: string[] = []
  globalThis.fetch = mock(async input => {
    const url = new URL(input.toString())
    requests.push(url.toString())
    return Response.json(
      url.pathname.endsWith("/actions")
        ? {actions: [], address_book: {}, metadata: {}}
        : {transactions: [], address_book: {}},
    )
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await client.getAccountTransactions("EQAddress", 25, 10, "asc")
    await client.getAccountActions("EQAddress", 15, 5, "asc")

    expect(requests).toEqual([
      "https://toncenter.example/api/v3/transactions?account=EQAddress&limit=25&offset=10&sort=asc",
      "https://toncenter.example/api/v3/actions?account=EQAddress&limit=15&offset=5&sort=asc",
    ])
  } finally {
    globalThis.fetch = originalFetch
  }
})

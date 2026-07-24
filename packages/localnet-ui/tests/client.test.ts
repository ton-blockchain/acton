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

test("wallet DNS lookup returns every domain for the requested address", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  const address = "UQDYzZmfsrGzhObKJUw4gzdeIxEai3jAFbiGKGwxvxHinf4K"
  const domains = [
    "monk.t.me",
    "wolf.t.me",
    "saint.t.me",
    "viking.t.me",
    "durovloh.ton",
    "puppeteer.ton",
    "upbanking.t.me",
    "ton-rooster.ton",
    "yourtonismy.ton",
    "dubaigoodbye.ton",
    "durovscammer.ton",
    "xn--037ha7bb.ton",
    "tg-tonloveton.ton",
    "wetrustinton.t.me",
  ] as const
  globalThis.fetch = mock(async input => {
    requests.push(new URL(input.toString()))
    return Response.json({
      records: domains.map(domain => ({domain})),
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await expect(client.getWalletDnsNames(address)).resolves.toEqual(domains)
    expect(requests).toHaveLength(1)
    expect(requests[0]?.pathname).toBe("/api/v3/dns/records")
    expect(requests[0]?.searchParams.get("wallet")).toBe(address)
    expect(requests[0]?.searchParams.get("limit")).toBe("1000")
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

test("localnet message submission uses the endpoint for each message type", async () => {
  const originalFetch = globalThis.fetch
  const requests: Array<{readonly url: URL; readonly init?: RequestInit}> = []
  globalThis.fetch = mock(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(input.toString())
    requests.push({url, init})
    return Response.json({
      ok: true,
      result: {
        hash: url.pathname.endsWith("/sendBocReturnHash") ? "external-hash" : "internal-hash",
      },
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await expect(client.sendInternalMessage("internal-boc")).resolves.toBe("internal-hash")
    await expect(client.sendExternalMessage("external-boc")).resolves.toBe("external-hash")

    expect(requests.map(request => request.url.pathname)).toEqual([
      "/api/acton_sendInternalMessage",
      "/api/v2/sendBocReturnHash",
    ])
    expect(requests.map(request => JSON.parse(String(request.init?.body)))).toEqual([
      {boc: "internal-boc"},
      {boc: "external-boc"},
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

test("transaction lookup requests one full transaction by hash", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  globalThis.fetch = mock(async input => {
    requests.push(new URL(input.toString()))
    return Response.json({transactions: [{hash: "transaction-hash"}], address_book: {}})
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    const result = await client.getTransactionByHash("requested-hash")

    expect({
      request: requests[0]?.toString(),
      result,
    }).toMatchInlineSnapshot(`
      {
        "request": "https://toncenter.example/api/v3/transactions?hash=requested-hash&limit=1",
        "result": {
          "address_book": {},
          "transactions": [
            {
              "hash": "transaction-hash",
            },
          ],
        },
      }
    `)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("NFT metadata preserves scam flags and excludes flagged or registered NSFW items", async () => {
  const originalFetch = globalThis.fetch
  globalThis.fetch = mock(async () =>
    Response.json({
      nft_items: [
        {
          address: "0:nft",
          code_hash: "code-hash",
          content: {},
          data_hash: "data-hash",
          index: "3",
          init: true,
          last_transaction_lt: "42",
          on_sale: false,
        },
        {
          address: "0:nsfw",
          code_hash: "nsfw-code-hash",
          content: {},
          data_hash: "nsfw-data-hash",
          index: "4",
          init: true,
          last_transaction_lt: "43",
          on_sale: false,
        },
        {
          address: "0:registered-nsfw",
          code_hash: "registered-code-hash",
          content: {
            _image_small:
              "https://proxy.toncenter.com/F0W0fr2CnSPVMdgFNe9x87X1TkFGKz7rUBtHpWmNXwc/pr:small/bG9jYWw6Ly8vc2hhMjU2L2VhZDllM2M1ZjI2MDc4NWU4ODUyYzBkY2E3YWQxZmQ3ZTY2OTBiMDMwMDlhMTU4YTg0OTI0M2U1OTY4NWFhN2Q",
          },
          data_hash: "registered-data-hash",
          index: "5",
          init: true,
          last_transaction_lt: "44",
          on_sale: false,
        },
      ],
      metadata: {
        "0:nft": {
          token_info: [
            {
              type: "nft_items",
              name: "Flagged NFT",
              is_nsfw: false,
              is_scam: true,
            },
          ],
        },
        "0:nsfw": {
          token_info: [
            {
              type: "nft_items",
              name: "Hidden NFT",
              is_nsfw: true,
              is_scam: false,
            },
          ],
        },
      },
    }),
  ) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    expect(await client.getNftItems({address: ["0:nft"]})).toMatchInlineSnapshot(`
      [
        {
          "address": "0:nft",
          "code_hash": "code-hash",
          "content": {
            "name": "Flagged NFT",
          },
          "data_hash": "data-hash",
          "index": "3",
          "init": true,
          "is_nsfw": false,
          "is_scam": true,
          "last_transaction_lt": "42",
          "on_sale": false,
        },
      ]
    `)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("localnet state and checkpoint methods transfer JSON through the control API", async () => {
  const originalFetch = globalThis.fetch
  const requests: Array<{readonly url: URL; readonly init?: RequestInit}> = []
  globalThis.fetch = mock(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(input.toString())
    requests.push({url, init})

    if (url.pathname.endsWith("/acton_dumpState")) {
      return new Response('{"version":1,"kind":"state"}', {
        headers: {"Content-Type": "application/json"},
      })
    }
    if (url.pathname.endsWith("/acton_exportCheckpoint")) {
      return new Response('{"version":1,"kind":"checkpoint"}', {
        headers: {"Content-Type": "application/json"},
      })
    }
    if (url.pathname.endsWith("/acton_listCheckpoints")) {
      return Response.json({
        ok: true,
        result: [{name: "before-deploy", block_seqno: 7}],
      })
    }
    if (url.pathname.endsWith("/acton_clearCheckpoints")) {
      return Response.json({ok: true, result: {deleted: 1}})
    }
    if (url.pathname.endsWith("/acton_loadState")) {
      return Response.json({ok: true, result: null})
    }
    if (url.pathname.endsWith("/acton_importCheckpoint")) {
      return Response.json({
        ok: true,
        result: {name: url.searchParams.get("name"), block_seqno: 7},
      })
    }

    const body = JSON.parse(String(init?.body)) as {name?: string}
    return Response.json({
      ok: true,
      result: {
        name: body.name ?? url.searchParams.get("name"),
        block_seqno: 7,
      },
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "http://localhost:8081/api/v2",
      v3BaseUrl: "http://localhost:8081/api/v3",
      addressNameBaseUrl: "http://localhost:8081",
      localnetApiToken: "test-token",
    })
    const state = new Blob(['{"version":1}'], {type: "application/json"})

    expect(await (await client.downloadState()).text()).toContain('"kind":"state"')
    await expect(client.loadState(state)).resolves.toBeUndefined()
    await expect(client.createCheckpoint("before-deploy")).resolves.toEqual({
      name: "before-deploy",
      block_seqno: 7,
    })
    await expect(client.listCheckpoints()).resolves.toEqual([
      {name: "before-deploy", block_seqno: 7},
    ])
    await expect(client.restoreCheckpoint("before-deploy")).resolves.toEqual({
      name: "before-deploy",
      block_seqno: 7,
    })
    expect(await (await client.downloadCheckpoint("before-deploy")).text()).toContain(
      '"kind":"checkpoint"',
    )
    await expect(client.importCheckpoint("imported", state)).resolves.toEqual({
      name: "imported",
      block_seqno: 7,
    })
    await expect(client.deleteCheckpoint("before-deploy")).resolves.toEqual({
      name: "before-deploy",
      block_seqno: 7,
    })
    await expect(client.clearCheckpoints()).resolves.toBe(1)

    expect(requests.map(request => request.url.pathname)).toEqual([
      "/acton_dumpState",
      "/acton_loadState",
      "/acton_createCheckpoint",
      "/acton_listCheckpoints",
      "/acton_restoreCheckpoint",
      "/acton_exportCheckpoint",
      "/acton_importCheckpoint",
      "/acton_deleteCheckpoint",
      "/acton_clearCheckpoints",
    ])
    expect(requests[6]?.url.searchParams.get("name")).toBe("imported")
    expect(requests[6]?.url.searchParams.get("force")).toBe("false")
    for (const request of requests) {
      expect(new Headers(request.init?.headers).get("Authorization")).toBe("Bearer test-token")
    }
  } finally {
    globalThis.fetch = originalFetch
  }
})

import {expect, mock, test} from "bun:test"

import {onRequest as onGetShardsRequest} from "../functions/api/toncenter/[network]/v2/getShards"
import {onRequest as onBlocksRequest} from "../functions/api/toncenter/[network]/v3/blocks"
import {onRequest as onTransactionsRequest} from "../functions/api/toncenter/[network]/v3/transactions"
import {installMemoryEdgeCache} from "./toncenterProxyTestUtils"

test("historical Toncenter blocks are normalized and cached for seven days", async () => {
  const originalFetch = globalThis.fetch
  const cache = installMemoryEdgeCache()
  const upstreamRequests: Array<{readonly url: string; readonly apiKey: string | null}> = []
  const backgroundTasks: Promise<unknown>[] = []

  globalThis.fetch = mock((input: RequestInfo | URL, init?: RequestInit) => {
    const request = new Request(input, init)
    upstreamRequests.push({
      url: request.url,
      apiKey: request.headers.get("X-API-Key"),
    })
    return Promise.resolve(
      Response.json({
        blocks: [
          {
            workchain: -1,
            shard: "8000000000000000",
            seqno: 42,
          },
        ],
      }),
    )
  }) as typeof fetch

  const context = {
    request: new Request(
      "https://actonscan.example/api/toncenter/mainnet/v3/blocks?limit=01&seqno=00042&shard=8000000000000000&workchain=-1",
    ),
    env: {
      TONCENTER_MAINNET_API_KEY: "server-mainnet-key",
    },
    params: {network: "mainnet"},
    waitUntil(promise: Promise<unknown>) {
      backgroundTasks.push(promise)
    },
  }

  try {
    const first = await onBlocksRequest(context)
    const firstBody = await first.json()
    await Promise.all(backgroundTasks)
    const second = await onBlocksRequest(context)
    const secondBody = await second.json()

    expect({
      upstreamRequests,
      first: {
        status: first.status,
        cache: first.headers.get("x-actonscan-cache"),
        cacheControl: first.headers.get("cache-control"),
        body: firstBody,
      },
      second: {
        status: second.status,
        cache: second.headers.get("x-actonscan-cache"),
        serverTimingIsCacheHit: second.headers.get("server-timing") === 'edge;desc="HIT"',
        body: secondBody,
      },
      cacheKeys: [...cache.responses.keys()],
    }).toMatchInlineSnapshot(`
      {
        "cacheKeys": [
          "https://actonscan.example/api/toncenter/mainnet/v3/blocks?workchain=-1&shard=8000000000000000&seqno=42&limit=1",
        ],
        "first": {
          "body": {
            "blocks": [
              {
                "seqno": 42,
                "shard": "8000000000000000",
                "workchain": -1,
              },
            ],
          },
          "cache": "MISS",
          "cacheControl": "public, max-age=300, s-maxage=604800, immutable",
          "status": 200,
        },
        "second": {
          "body": {
            "blocks": [
              {
                "seqno": 42,
                "shard": "8000000000000000",
                "workchain": -1,
              },
            ],
          },
          "cache": "HIT",
          "serverTimingIsCacheHit": true,
          "status": 200,
        },
        "upstreamRequests": [
          {
            "apiKey": "server-mainnet-key",
            "url": "https://toncenter.com/api/v3/blocks?workchain=-1&shard=8000000000000000&seqno=42&limit=1",
          },
        ],
      }
    `)
  } finally {
    globalThis.fetch = originalFetch
    cache.restore()
  }
})

test("latest block lists use a short edge cache and empty results are not cached", async () => {
  const originalFetch = globalThis.fetch
  const cache = installMemoryEdgeCache()
  const backgroundTasks: Promise<unknown>[] = []
  let upstreamRequestCount = 0

  globalThis.fetch = mock(() => {
    upstreamRequestCount += 1
    return Promise.resolve(
      Response.json({
        blocks:
          upstreamRequestCount === 1 ? [] : [{workchain: 0, shard: "8000000000000000", seqno: 84}],
      }),
    )
  }) as typeof fetch

  const context = {
    request: new Request(
      "https://actonscan.example/api/toncenter/testnet/v3/blocks?sort=desc&limit=8&workchain=0",
    ),
    env: {},
    params: {network: "testnet"},
    waitUntil(promise: Promise<unknown>) {
      backgroundTasks.push(promise)
    },
  }

  try {
    const empty = await onBlocksRequest(context)
    const populated = await onBlocksRequest(context)
    await Promise.all(backgroundTasks)
    const cached = await onBlocksRequest(context)
    const duplicate = await onBlocksRequest({
      ...context,
      request: new Request(
        "https://actonscan.example/api/toncenter/testnet/v3/blocks?workchain=0&workchain=-1",
      ),
    })

    expect({
      empty: {
        cache: empty.headers.get("x-actonscan-cache"),
        cacheControl: empty.headers.get("cache-control"),
      },
      populated: {
        cache: populated.headers.get("x-actonscan-cache"),
        cacheControl: populated.headers.get("cache-control"),
      },
      cached: {
        cache: cached.headers.get("x-actonscan-cache"),
        body: await cached.json(),
      },
      duplicate: {
        status: duplicate.status,
        body: await duplicate.json(),
      },
      upstreamRequestCount,
      cacheKeys: [...cache.responses.keys()],
    }).toMatchInlineSnapshot(`
      {
        "cacheKeys": [
          "https://actonscan.example/api/toncenter/testnet/v3/blocks?workchain=0&limit=8&sort=desc",
        ],
        "cached": {
          "body": {
            "blocks": [
              {
                "seqno": 84,
                "shard": "8000000000000000",
                "workchain": 0,
              },
            ],
          },
          "cache": "HIT",
        },
        "duplicate": {
          "body": {
            "error": "Only one workchain query parameter is allowed",
          },
          "status": 400,
        },
        "empty": {
          "cache": "MISS",
          "cacheControl": "no-store",
        },
        "populated": {
          "cache": "MISS",
          "cacheControl": "public, max-age=0, s-maxage=2, must-revalidate",
        },
        "upstreamRequestCount": 2,
      }
    `)
  } finally {
    globalThis.fetch = originalFetch
    cache.restore()
  }
})

test("block transactions and masterchain shards share the historical cache policy", async () => {
  const originalFetch = globalThis.fetch
  const cache = installMemoryEdgeCache()
  const upstreamRequests: Array<{readonly url: string; readonly apiKey: string | null}> = []
  const backgroundTasks: Promise<unknown>[] = []

  globalThis.fetch = mock((input: RequestInfo | URL, init?: RequestInit) => {
    const request = new Request(input, init)
    upstreamRequests.push({
      url: request.url,
      apiKey: request.headers.get("X-API-Key"),
    })
    return Promise.resolve(
      request.url.includes("/api/v2/getShards")
        ? Response.json({
            ok: true,
            result: {
              shards: [
                {
                  workchain: 0,
                  shard: "-9223372036854775808",
                  seqno: 91,
                },
              ],
            },
          })
        : Response.json({
            transactions: [{hash: "block-transaction"}],
            address_book: {},
          }),
    )
  }) as typeof fetch

  const context = (url: string) => ({
    request: new Request(url),
    env: {TONCENTER_TESTNET_API_KEY: "server-testnet-key"},
    params: {network: "testnet"},
    waitUntil(promise: Promise<unknown>) {
      backgroundTasks.push(promise)
    },
  })

  try {
    const transactionsUrl =
      "https://actonscan.example/api/toncenter/testnet/v3/transactions?offset=00100&limit=0100&seqno=00042&shard=8000000000000000&workchain=-1"
    const shardsUrl = "https://actonscan.example/api/toncenter/testnet/v2/getShards?seqno=081928675"
    const transactionsMiss = await onTransactionsRequest(context(transactionsUrl))
    const shardsMiss = await onGetShardsRequest(context(shardsUrl))
    await Promise.all(backgroundTasks)
    const transactionsHit = await onTransactionsRequest(context(transactionsUrl))
    const shardsHit = await onGetShardsRequest(context(shardsUrl))
    const invalidTransactions = await onTransactionsRequest(
      context(
        "https://actonscan.example/api/toncenter/testnet/v3/transactions?workchain=-1&shard=8000000000000000",
      ),
    )
    const invalidShards = await onGetShardsRequest(
      context("https://actonscan.example/api/toncenter/testnet/v2/getShards?seqno=42&seqno=43"),
    )
    const unscopedBlockSeqno = await onBlocksRequest(
      context("https://actonscan.example/api/toncenter/testnet/v3/blocks?seqno=42"),
    )
    const excessiveTransactionLimit = await onTransactionsRequest(
      context(
        "https://actonscan.example/api/toncenter/testnet/v3/transactions?workchain=-1&shard=8000000000000000&seqno=42&limit=1001",
      ),
    )
    const outOfRangeShardSeqno = await onGetShardsRequest(
      context("https://actonscan.example/api/toncenter/testnet/v2/getShards?seqno=2147483648"),
    )

    expect({
      misses: [
        {
          cache: transactionsMiss.headers.get("x-actonscan-cache"),
          cacheControl: transactionsMiss.headers.get("cache-control"),
        },
        {
          cache: shardsMiss.headers.get("x-actonscan-cache"),
          cacheControl: shardsMiss.headers.get("cache-control"),
        },
      ],
      hits: [
        {
          cache: transactionsHit.headers.get("x-actonscan-cache"),
          body: await transactionsHit.json(),
        },
        {
          cache: shardsHit.headers.get("x-actonscan-cache"),
          body: await shardsHit.json(),
        },
      ],
      invalid: [
        {
          status: invalidTransactions.status,
          body: await invalidTransactions.json(),
        },
        {
          status: invalidShards.status,
          body: await invalidShards.json(),
        },
        {
          status: unscopedBlockSeqno.status,
          body: await unscopedBlockSeqno.json(),
        },
        {
          status: excessiveTransactionLimit.status,
          body: await excessiveTransactionLimit.json(),
        },
        {
          status: outOfRangeShardSeqno.status,
          body: await outOfRangeShardSeqno.json(),
        },
      ],
      upstreamRequests,
      cacheKeys: [...cache.responses.keys()],
    }).toMatchInlineSnapshot(`
      {
        "cacheKeys": [
          "https://actonscan.example/api/toncenter/testnet/v3/transactions?workchain=-1&shard=8000000000000000&seqno=42&limit=100&offset=100",
          "https://actonscan.example/api/toncenter/testnet/v2/getShards?seqno=81928675",
        ],
        "hits": [
          {
            "body": {
              "address_book": {},
              "transactions": [
                {
                  "hash": "block-transaction",
                },
              ],
            },
            "cache": "HIT",
          },
          {
            "body": {
              "ok": true,
              "result": {
                "shards": [
                  {
                    "seqno": 91,
                    "shard": "-9223372036854775808",
                    "workchain": 0,
                  },
                ],
              },
            },
            "cache": "HIT",
          },
        ],
        "invalid": [
          {
            "body": {
              "error": "Exactly one seqno query parameter is required",
            },
            "status": 400,
          },
          {
            "body": {
              "error": "Exactly one int32 seqno query parameter is required",
            },
            "status": 400,
          },
          {
            "body": {
              "error": "seqno must be provided with workchain and shard",
            },
            "status": 400,
          },
          {
            "body": {
              "error": "Invalid limit query parameter",
            },
            "status": 400,
          },
          {
            "body": {
              "error": "Exactly one int32 seqno query parameter is required",
            },
            "status": 400,
          },
        ],
        "misses": [
          {
            "cache": "MISS",
            "cacheControl": "public, max-age=300, s-maxage=604800, immutable",
          },
          {
            "cache": "MISS",
            "cacheControl": "public, max-age=300, s-maxage=604800, immutable",
          },
        ],
        "upstreamRequests": [
          {
            "apiKey": "server-testnet-key",
            "url": "https://testnet.toncenter.com/api/v3/transactions?workchain=-1&shard=8000000000000000&seqno=42&limit=100&offset=100",
          },
          {
            "apiKey": "server-testnet-key",
            "url": "https://testnet.toncenter.com/api/v2/getShards?seqno=81928675",
          },
        ],
      }
    `)
  } finally {
    globalThis.fetch = originalFetch
    cache.restore()
  }
})

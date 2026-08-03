import {expect, mock, test} from "bun:test"

import {onRequest} from "../functions/api/toncenter/[network]/v3/traces"
import {installMemoryEdgeCache} from "./toncenterProxyTestUtils"

const TRANSACTION_HASH = "a".repeat(64)

test("complete Toncenter traces are cached by network and transaction hash", async () => {
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
        traces: [
          {
            trace_id: "trace",
            is_incomplete: false,
            trace_info: {pending_messages: 0},
          },
        ],
      }),
    )
  }) as typeof fetch

  const context = {
    request: new Request(
      `https://actonscan.example/api/toncenter/testnet/v3/traces?include_actions=true&tx_hash=${TRANSACTION_HASH.toUpperCase()}`,
    ),
    env: {
      TONCENTER_TESTNET_API_KEY: "server-testnet-key",
    },
    params: {network: "testnet"},
    waitUntil(promise: Promise<unknown>) {
      backgroundTasks.push(promise)
    },
  }

  try {
    const first = await onRequest(context)
    const firstBody = await first.json()
    await Promise.all(backgroundTasks)
    const second = await onRequest(context)
    const secondBody = await second.json()

    expect({
      upstreamRequests,
      first: {
        status: first.status,
        cache: first.headers.get("x-actonscan-cache"),
        cacheControl: first.headers.get("cache-control"),
        serverTimingPrefix: first.headers.get("server-timing")?.split("=")[0],
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
          "https://actonscan.example/api/toncenter/testnet/v3/traces?tx_hash=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&include_actions=true",
        ],
        "first": {
          "body": {
            "traces": [
              {
                "is_incomplete": false,
                "trace_id": "trace",
                "trace_info": {
                  "pending_messages": 0,
                },
              },
            ],
          },
          "cache": "MISS",
          "cacheControl": "public, max-age=300, s-maxage=604800",
          "serverTimingPrefix": "toncenter;dur",
          "status": 200,
        },
        "second": {
          "body": {
            "traces": [
              {
                "is_incomplete": false,
                "trace_id": "trace",
                "trace_info": {
                  "pending_messages": 0,
                },
              },
            ],
          },
          "cache": "HIT",
          "serverTimingIsCacheHit": true,
          "status": 200,
        },
        "upstreamRequests": [
          {
            "apiKey": "server-testnet-key",
            "url": "https://testnet.toncenter.com/api/v3/traces?tx_hash=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&include_actions=true",
          },
        ],
      }
    `)
  } finally {
    globalThis.fetch = originalFetch
    cache.restore()
  }
})

test("invalid and incomplete trace responses are not cached", async () => {
  const originalFetch = globalThis.fetch
  const cache = installMemoryEdgeCache()
  let upstreamRequestCount = 0

  globalThis.fetch = mock(() => {
    upstreamRequestCount += 1
    return Promise.resolve(
      Response.json({
        traces: [
          {
            trace_id: "pending-trace",
            is_incomplete: true,
            trace_info: {pending_messages: 1},
          },
        ],
      }),
    )
  }) as typeof fetch

  const request = (hash: string) =>
    onRequest({
      request: new Request(
        `https://actonscan.example/api/toncenter/mainnet/v3/traces?tx_hash=${hash}`,
      ),
      env: {},
      params: {network: "mainnet"},
      waitUntil() {
        // Incomplete traces must not schedule cache writes.
      },
    })

  try {
    const invalid = await request("not-a-transaction-hash")
    const firstIncomplete = await request(TRANSACTION_HASH)
    const secondIncomplete = await request(TRANSACTION_HASH)

    expect({
      invalid: {
        status: invalid.status,
        body: await invalid.json(),
      },
      incompleteResponses: [
        {
          cache: firstIncomplete.headers.get("x-actonscan-cache"),
          cacheControl: firstIncomplete.headers.get("cache-control"),
        },
        {
          cache: secondIncomplete.headers.get("x-actonscan-cache"),
          cacheControl: secondIncomplete.headers.get("cache-control"),
        },
      ],
      upstreamRequestCount,
      cachePutCount: cache.putCount,
    }).toMatchInlineSnapshot(`
      {
        "cachePutCount": 0,
        "incompleteResponses": [
          {
            "cache": "MISS",
            "cacheControl": "no-store",
          },
          {
            "cache": "MISS",
            "cacheControl": "no-store",
          },
        ],
        "invalid": {
          "body": {
            "error": "tx_hash must be a 64-character hexadecimal transaction hash",
          },
          "status": 400,
        },
        "upstreamRequestCount": 2,
      }
    `)
  } finally {
    globalThis.fetch = originalFetch
    cache.restore()
  }
})

test("Toncenter rate-limit status survives an invalid JSON response", async () => {
  const originalFetch = globalThis.fetch
  const cache = installMemoryEdgeCache()
  globalThis.fetch = mock(() =>
    Promise.resolve(
      new Response("rate limited", {
        status: 429,
        headers: {"retry-after": "10"},
      }),
    ),
  ) as typeof fetch

  try {
    const response = await onRequest({
      request: new Request(
        `https://actonscan.example/api/toncenter/mainnet/v3/traces?tx_hash=${TRANSACTION_HASH}`,
      ),
      env: {},
      params: {network: "mainnet"},
      waitUntil() {
        throw new Error("Rate-limit responses must not be cached")
      },
    })

    expect({
      status: response.status,
      retryAfter: response.headers.get("retry-after"),
      cacheControl: response.headers.get("cache-control"),
      body: await response.json(),
      cachePutCount: cache.putCount,
    }).toMatchInlineSnapshot(`
      {
        "body": {
          "error": "Toncenter returned an invalid JSON response",
        },
        "cacheControl": "no-store",
        "cachePutCount": 0,
        "retryAfter": "10",
        "status": 429,
      }
    `)
  } finally {
    globalThis.fetch = originalFetch
    cache.restore()
  }
})

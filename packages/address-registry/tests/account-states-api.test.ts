import {expect, mock, test} from "bun:test"

import {readNetworkAccountStates} from "../scripts/network/account-states-api.ts"

const ADDRESSES = Array.from(
  {length: 101},
  (_, index) => `0:${index.toString(16).padStart(64, "0")}`,
)

test("batches both networks and retries a rate-limited request", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  let testnetRequests = 0
  globalThis.fetch = mock(input => {
    const url = new URL(input.toString())
    requests.push(url)

    if (url.hostname === "testnet.toncenter.com") {
      testnetRequests += 1
      if (testnetRequests === 1) {
        return Promise.resolve(new Response(null, {status: 429, headers: {"Retry-After": "0"}}))
      }
    }

    return Promise.resolve(
      Response.json({
        accounts: url.searchParams.getAll("address").map(address => ({address, status: "active"})),
      }),
    )
  }) as typeof fetch

  try {
    expect(await readNetworkAccountStates(ADDRESSES, "api-key")).toEqual({
      mainnet: ADDRESSES.map(address => ({address, status: "active"})),
      testnet: ADDRESSES.map(address => ({address, status: "active"})),
    })
    expect(requests.map(url => url.hostname)).toEqual([
      "toncenter.com",
      "testnet.toncenter.com",
      "testnet.toncenter.com",
      "toncenter.com",
      "testnet.toncenter.com",
    ])
    expect(requests.map(url => url.searchParams.getAll("address").length)).toEqual([
      100, 100, 100, 1, 1,
    ])
    for (const url of requests) {
      expect(url.searchParams.get("include_boc")).toBe("false")
    }
  } finally {
    globalThis.fetch = originalFetch
  }
})

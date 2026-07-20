import {expect, mock, test} from "bun:test"

import {TonClient} from "../src/explorer/api/client"

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

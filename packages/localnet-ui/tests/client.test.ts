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

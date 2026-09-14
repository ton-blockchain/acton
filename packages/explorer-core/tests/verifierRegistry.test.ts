import {expect, mock, spyOn, test} from "bun:test"

import {VerifierMetadataRegistry} from "../src/metadata/verifierRegistry"

const mockFetch = (
  implementation: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>,
) => Object.assign(mock(implementation), {preconnect: globalThis.fetch.preconnect})

const CODE_HASH = "a".repeat(64)

test("stalled verifier ABI requests time out without blocking metadata resolution", async () => {
  const originalFetch = globalThis.fetch
  let requestSignal: AbortSignal | null | undefined
  globalThis.fetch = mockFetch((_input, init) => {
    requestSignal = init?.signal
    return rejectWhenAborted(requestSignal)
  })

  try {
    const registry = new VerifierMetadataRegistry({requestTimeoutMs: 5})

    expect(await registry.getCompilerAbis([CODE_HASH])).toEqual({[CODE_HASH]: null})
    expect(requestSignal?.aborted).toBe(true)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("a timed-out verifier lookup is retried instead of being cached as missing", async () => {
  const originalFetch = globalThis.fetch
  let requestCount = 0
  globalThis.fetch = mockFetch((_input, init) => {
    requestCount += 1
    if (requestCount === 1) {
      return rejectWhenAborted(init?.signal)
    }
    return Promise.resolve(
      Response.json({
        items: [{code_hash: CODE_HASH, abi: {contract_name: "RecoveredContract"}}],
      }),
    )
  })

  try {
    const registry = new VerifierMetadataRegistry({requestTimeoutMs: 5})

    expect(await registry.getCompilerAbis([CODE_HASH])).toEqual({[CODE_HASH]: null})
    const recovered = await registry.getCompilerAbis([CODE_HASH])

    expect(recovered[CODE_HASH]?.compiler_abi).toEqual({contract_name: "RecoveredContract"})
    expect(requestCount).toBe(2)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("stalled verifier source requests also fall back after the request deadline", async () => {
  const originalFetch = globalThis.fetch
  let requestSignal: AbortSignal | null | undefined
  globalThis.fetch = mockFetch((_input, init) => {
    requestSignal = init?.signal
    return rejectWhenAborted(requestSignal)
  })

  try {
    const registry = new VerifierMetadataRegistry({requestTimeoutMs: 5})

    expect(await registry.getSource({codeHash: CODE_HASH})).toEqual({
      code_hash: CODE_HASH,
      verified: false,
      bundle: null,
    })
    expect(requestSignal?.aborted).toBe(true)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("newly verified ABIs and sources become visible without reloading the registry", async () => {
  const originalFetch = globalThis.fetch
  const start = Date.now()
  const clock = spyOn(Date, "now").mockReturnValue(start)
  let verified = false
  let requestCount = 0
  const abi = {contract_name: "NewContract"}
  const source = {code_hash: CODE_HASH, verified: true, bundle: {files: []}}
  globalThis.fetch = mockFetch(async input => {
    requestCount += 1
    if (!verified) {
      return Response.json({error: "not found"}, {status: 404})
    }
    return Response.json(
      String(input).includes("/abi?") ? {items: [{code_hash: CODE_HASH, abi}]} : source,
    )
  })
  try {
    const registry = new VerifierMetadataRegistry()
    expect(await registry.getCompilerAbis([CODE_HASH])).toEqual({[CODE_HASH]: null})
    expect((await registry.getSource({codeHash: CODE_HASH})).verified).toBe(false)
    verified = true
    clock.mockReturnValue(start + 59_999)
    expect(await registry.getCompilerAbis([CODE_HASH])).toEqual({[CODE_HASH]: null})
    expect(requestCount).toBe(2)
    clock.mockReturnValue(start + 60_000)
    expect((await registry.getCompilerAbis([CODE_HASH]))[CODE_HASH]?.compiler_abi).toEqual(abi)
    expect(await registry.getSource({codeHash: CODE_HASH})).toEqual(source)
    clock.mockReturnValue(start + 120_000)
    await registry.getCompilerAbis([CODE_HASH])
    await registry.getSource({codeHash: CODE_HASH})
    expect(requestCount).toBe(4)
  } finally {
    clock.mockRestore()
    globalThis.fetch = originalFetch
  }
})

test.each([404, 200])("missing verifier ABIs are cached for HTTP %s responses", async status => {
  const originalFetch = globalThis.fetch
  const fetch = mockFetch(async () => Response.json({items: []}, {status}))
  globalThis.fetch = fetch
  try {
    const registry = new VerifierMetadataRegistry()
    expect(await registry.getCompilerAbis([CODE_HASH])).toEqual({[CODE_HASH]: null})
    expect(await registry.getCompilerAbis([CODE_HASH.toUpperCase()])).toEqual({
      [CODE_HASH.toUpperCase()]: null,
    })
    expect(fetch).toHaveBeenCalledTimes(1)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("overlapping verifier ABI lookups share a request for the same normalized hash", async () => {
  const originalFetch = globalThis.fetch
  const response = Promise.withResolvers<Response>()
  const fetch = mockFetch(() => response.promise)
  globalThis.fetch = fetch
  try {
    const registry = new VerifierMetadataRegistry()
    const first = registry.getCompilerAbis([CODE_HASH, CODE_HASH.toUpperCase()])
    const second = registry.getCompilerAbis([CODE_HASH])
    expect(fetch).toHaveBeenCalledTimes(1)
    response.resolve(Response.json({error: "not found"}, {status: 404}))
    expect(await first).toEqual({[CODE_HASH]: null, [CODE_HASH.toUpperCase()]: null})
    expect(await second).toEqual({[CODE_HASH]: null})
    await registry.getCompilerAbis([CODE_HASH])
    expect(fetch).toHaveBeenCalledTimes(1)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test.each([429, 500])("HTTP %s verifier failures do not become cached misses", async status => {
  const originalFetch = globalThis.fetch
  const fetch = mockFetch(async () => Response.json({error: "unavailable"}, {status}))
  globalThis.fetch = fetch
  try {
    const registry = new VerifierMetadataRegistry()
    expect(await registry.getCompilerAbis([CODE_HASH])).toEqual({[CODE_HASH]: null})
    fetch.mockImplementation(async () =>
      Response.json({items: [{code_hash: CODE_HASH, abi: {contract_name: "RecoveredContract"}}]}),
    )
    expect((await registry.getCompilerAbis([CODE_HASH]))[CODE_HASH]?.compiler_abi).toEqual({
      contract_name: "RecoveredContract",
    })
    expect(fetch).toHaveBeenCalledTimes(2)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("address lookups refresh after a code upgrade even when a hash is cached", async () => {
  const originalFetch = globalThis.fetch
  let currentHash = CODE_HASH
  globalThis.fetch = mockFetch(async () =>
    Response.json({code_hash: currentHash, verified: true, bundle: {files: []}}),
  )
  try {
    const registry = new VerifierMetadataRegistry()
    const address = `0:${"1".repeat(64)}`
    expect((await registry.getSource({address})).code_hash).toBe(CODE_HASH)
    currentHash = "b".repeat(64)
    expect((await registry.getSource({address})).code_hash).toBe(currentHash)
    expect((await registry.getSource({address, codeHash: CODE_HASH})).verified).toBe(false)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("sources for an unrelated code hash are rejected and do not poison the cache", async () => {
  const originalFetch = globalThis.fetch
  let responseHash = "b".repeat(64)
  globalThis.fetch = mockFetch(async () =>
    Response.json({code_hash: responseHash, verified: true, bundle: {files: []}}),
  )
  try {
    const registry = new VerifierMetadataRegistry()
    expect(await registry.getSource({codeHash: CODE_HASH})).toEqual({
      code_hash: CODE_HASH,
      verified: false,
      bundle: null,
    })
    responseHash = CODE_HASH
    expect((await registry.getSource({codeHash: CODE_HASH})).verified).toBe(true)
  } finally {
    globalThis.fetch = originalFetch
  }
})

function rejectWhenAborted(signal: AbortSignal | null | undefined): Promise<Response> {
  return new Promise((_resolve, reject) => {
    if (!signal) {
      reject(new Error("Expected verifier request to have an AbortSignal"))
      return
    }
    const rejectWithReason = () => reject(signal.reason)
    if (signal.aborted) {
      rejectWithReason()
      return
    }
    signal.addEventListener("abort", rejectWithReason, {once: true})
  })
}

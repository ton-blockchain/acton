import {describe, expect, it} from "vitest"

import {normalizeRetraceHash, parseActonContractIds} from "./ActonRetraceUtils"

describe("Acton retrace utils", () => {
  it("parses contract ids from Acton.toml content", () => {
    const content = `
[contracts.Counter]
path = "contracts/counter.tolk"

[contracts."Jetton.Minter"] # quoted key with a dot
path = "contracts/jetton-minter.tolk"

[contracts."Jetton.Minter".test]
skip = true
`

    expect(parseActonContractIds(content)).toEqual(["Counter", "Jetton.Minter"])
  })

  it("normalizes retrace hashes and strips the 0x prefix", () => {
    const hash = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

    expect(normalizeRetraceHash(hash)).toBe(
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
  })

  it("rejects invalid retrace hashes", () => {
    expect(normalizeRetraceHash("")).toBeNull()
    expect(normalizeRetraceHash("0x1234")).toBeNull()
    expect(normalizeRetraceHash("zzzz")).toBeNull()
  })
})

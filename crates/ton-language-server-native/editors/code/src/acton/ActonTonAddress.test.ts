import {describe, expect, it} from "vitest"

import {
  createTonAddressExplorerUrl,
  findTonAddressMatches,
  normalizeTonExplorer,
} from "./ActonTonAddress"

describe("Acton TON address links", () => {
  it("finds raw TON addresses", () => {
    const address = "-1:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    const line = `Deployed at ${address}`

    expect(findTonAddressMatches(line)).toEqual([
      {
        address,
        startIndex: "Deployed at ".length,
        length: address.length,
        isTestnet: false,
      },
    ])
  })

  it("finds user-friendly mainnet and testnet addresses", () => {
    const mainnet = `EQ${"A".repeat(46)}`
    const testnet = `kQ${"A".repeat(46)}`
    const line = `${mainnet} -> ${testnet}`

    expect(findTonAddressMatches(line)).toEqual([
      {
        address: mainnet,
        startIndex: 0,
        length: mainnet.length,
        isTestnet: false,
      },
      {
        address: testnet,
        startIndex: mainnet.length + 4,
        length: testnet.length,
        isTestnet: true,
      },
    ])
  })

  it("skips long lines to avoid linking raw payloads", () => {
    const address = `EQ${"A".repeat(46)}`
    const line = `${"x".repeat(301)} ${address}`

    expect(findTonAddressMatches(line)).toEqual([])
  })

  it("builds explorer urls", () => {
    expect(
      createTonAddressExplorerUrl(
        "0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "tonscan",
        false,
      ),
    ).toBe(
      "https://tonscan.org/address/0%3A0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )

    expect(createTonAddressExplorerUrl("kQabc", "tonviewer", true)).toBe(
      "https://testnet.tonviewer.com/kQabc",
    )
  })

  it("normalizes explorer setting values", () => {
    expect(normalizeTonExplorer("tonviewer")).toBe("tonviewer")
    expect(normalizeTonExplorer("tonscan")).toBe("tonscan")
    expect(normalizeTonExplorer("unknown")).toBe("tonscan")
    expect(normalizeTonExplorer(undefined)).toBe("tonscan")
  })
})

import {describe, expect, test} from "bun:test"

import {getExtraCurrencyMetadata} from "../src/api/extraCurrency"

describe("extra-currency metadata", () => {
  test("returns known display metadata", () => {
    expect(getExtraCurrencyMetadata(-17)).toEqual({
      decimals: 9,
      origin: {
        label: "Origin: TON zero state",
        linkLabel: "View zero-state source",
        source: {
          kind: "external",
          url: "https://github.com/ton-blockchain/ton/blob/70d73c87ad489f422a09f64a78f4fdc74edcb596/crypto/smartcont/gen-zerostate-test.fif#L203",
        },
      },
      symbol: "UNDEFINED",
    })
    expect(getExtraCurrencyMetadata(239)).toEqual({
      decimals: 5,
      origin: {
        label: "Origin: TON zero state",
        linkLabel: "View zero-state source",
        source: {
          kind: "external",
          url: "https://github.com/ton-blockchain/ton/blob/70d73c87ad489f422a09f64a78f4fdc74edcb596/crypto/smartcont/gen-zerostate-test.fif#L203",
        },
      },
      symbol: "FMS",
    })
    expect(getExtraCurrencyMetadata(100)).toEqual({
      decimals: 8,
      origin: {
        label: "Origin: Testnet Extra Currency Minter · 5 Dec 2024",
        linkLabel: "View mint transaction in ActonScan",
        source: {
          kind: "transaction",
          hash: "d73eff828d5bbeb748400cb5112a06a95a08f1e70c267fdc1ce9d23f799ec928",
        },
      },
      symbol: "ECHIDNA",
    })
  })

  test("uses the signed currency ID for unknown display metadata", () => {
    expect(getExtraCurrencyMetadata(-18)).toEqual({decimals: 9, symbol: "$-18"})
  })
})

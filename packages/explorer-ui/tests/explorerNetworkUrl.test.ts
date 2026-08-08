import {describe, expect, test} from "bun:test"

import {explorerNetworkSearch} from "../src/explorerNetworkUrl"

describe("explorerNetworkSearch", () => {
  test("adds mainnet to an otherwise empty URL", () => {
    expect(explorerNetworkSearch("", "mainnet")).toBe("network=mainnet")
  })

  test("keeps unrelated parameters when switching built-in networks", () => {
    expect(explorerNetworkSearch("?query=value&network=testnet", "mainnet")).toBe(
      "query=value&network=mainnet",
    )
    expect(explorerNetworkSearch("?query=value&network=mainnet", "testnet")).toBe(
      "query=value&network=testnet",
    )
  })

  test("removes the built-in network parameter for custom networks", () => {
    expect(explorerNetworkSearch("?query=value&network=mainnet", "custom:devnet")).toBe(
      "query=value",
    )
  })
})

import {expect, test} from "bun:test"

import mainnetJson from "../src/mainnet.json" with {type: "json"}
import testnetJson from "../src/testnet.json" with {type: "json"}
import {addresses, getMainnetAddresses, getTestnetAddresses} from "../src/addresses.ts"

const RAW_ADDRESS_PATTERN = /^-?\d+:[0-9a-f]{64}$/

test("TypeScript binding exposes both generated registries", () => {
  expect(addresses).toEqual([...mainnetJson, ...testnetJson])
  expect(getMainnetAddresses()).toBe(mainnetJson)
  expect(getTestnetAddresses()).toBe(testnetJson)
})

test("generated JSON files match the binding schema", () => {
  for (const registry of [mainnetJson, testnetJson]) {
    for (const entry of registry as readonly Record<string, unknown>[]) {
      expect(Object.keys(entry).toSorted()).toEqual(["address", "name"])
      expect(typeof entry.address).toBe("string")
      expect(typeof entry.name).toBe("string")
      expect(entry.address).toMatch(RAW_ADDRESS_PATTERN)
      expect(entry.name).not.toBe("")
    }
  }
})

test("generated addresses are unique within each network", () => {
  for (const registry of [mainnetJson, testnetJson]) {
    expect(registry.length).toBeGreaterThan(0)
    expect(new Set(registry.map(({address}) => address)).size).toBe(registry.length)
  }
})

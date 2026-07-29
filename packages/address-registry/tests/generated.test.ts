import {expect, test} from "bun:test"

import addressesJson from "../src/addresses.json" with {type: "json"}
import {addresses} from "../src/addresses.ts"

const RAW_ADDRESS_PATTERN = /^-?\d+:[0-9a-f]{64}$/

test("TypeScript binding exposes addresses.json", () => {
  expect(addresses).toBe(addressesJson)
})

test("generated JSON matches the binding schema", () => {
  for (const entry of addressesJson as readonly Record<string, unknown>[]) {
    expect(Object.keys(entry).toSorted()).toEqual(["address", "name"])
    expect(typeof entry.address).toBe("string")
    expect(typeof entry.name).toBe("string")
    expect(entry.address).toMatch(RAW_ADDRESS_PATTERN)
    expect(entry.name).not.toBe("")
  }
})

test("generated addresses are unique and sorted by raw address", () => {
  expect(addresses.length).toBeGreaterThan(0)
  expect(new Set(addresses.map(({address}) => address)).size).toBe(addresses.length)
  expect(addresses.map(({address}) => address)).toEqual(
    addresses.map(({address}) => address).toSorted(),
  )
})

import {expect, test} from "bun:test"

import {addresses} from "../src/addresses.ts"

test("generated addresses are unique and sorted by raw address", () => {
  expect(addresses.length).toBeGreaterThan(0)
  expect(new Set(addresses.map(({address}) => address)).size).toBe(addresses.length)
  expect(addresses.map(({address}) => address)).toEqual(
    addresses.map(({address}) => address).toSorted(),
  )
})

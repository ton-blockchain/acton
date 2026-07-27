import {describe, expect, test} from "bun:test"

import {mergeSources} from "../scripts/merge.ts"
import type {AddressSource} from "../scripts/sources/shared.ts"

const RAW_ZERO = `0:${"0".repeat(64)}`
const RAW_ONES = `0:${"1".repeat(64)}`
const RAW_TWOS = `0:${"2".repeat(64)}`

const source = (id: AddressSource["id"], addresses: AddressSource["addresses"]): AddressSource => ({
  id,
  urls: [],
  addresses,
})

describe("mergeSources without conflicts", () => {
  test("merges unique addresses", () => {
    expect(
      mergeSources([
        source("ton-assets", [{address: RAW_ZERO, name: "Alpha"}]),
        source("address-book", [{address: RAW_ONES, name: "Beta"}]),
      ]),
    ).toEqual({
      addresses: [
        {address: RAW_ZERO, name: "Alpha"},
        {address: RAW_ONES, name: "Beta"},
      ],
      conflicts: [],
    })
  })

  test("collapses equal names for the same address", () => {
    expect(
      mergeSources([
        source("ton-assets", [
          {address: RAW_ZERO, name: "Alpha"},
          {address: RAW_ZERO, name: "Alpha"},
        ]),
        source("address-book", [{address: RAW_ZERO, name: "Alpha"}]),
      ]),
    ).toEqual({
      addresses: [{address: RAW_ZERO, name: "Alpha"}],
      conflicts: [],
    })
  })
})

describe("mergeSources conflicts", () => {
  test("reports different names without choosing one", () => {
    expect(
      mergeSources([
        source("ton-assets", [
          {address: RAW_ZERO, name: "Alpha"},
          {address: RAW_TWOS, name: "Gamma"},
        ]),
        source("address-book", [{address: RAW_ZERO, name: "Beta"}]),
      ]),
    ).toEqual({
      addresses: [{address: RAW_TWOS, name: "Gamma"}],
      conflicts: [
        {
          address: RAW_ZERO,
          candidates: [
            {source: "ton-assets", name: "Alpha"},
            {source: "address-book", name: "Beta"},
          ],
        },
      ],
    })
  })
})

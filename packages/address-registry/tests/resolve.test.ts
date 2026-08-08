import {describe, expect, test} from "bun:test"

import type {AddressConflict} from "../scripts/merge.ts"
import {resolveConflicts} from "../scripts/resolve.ts"

const RAW_ZERO = `0:${"0".repeat(64)}`

const conflict: AddressConflict = {
  address: RAW_ZERO,
  candidates: [
    {source: "ton-assets", name: "Alpha"},
    {source: "address-book", name: "Beta"},
  ],
}

describe("resolveConflicts", () => {
  test("selects an exact source and name", () => {
    expect(
      resolveConflicts([conflict], [{address: RAW_ZERO, source: "address-book", name: "Beta"}]),
    ).toEqual({
      addresses: [{address: RAW_ZERO, name: "Beta"}],
      unresolved: [],
    })
  })

  test("keeps conflicts without resolutions", () => {
    expect(resolveConflicts([conflict], [])).toEqual({
      addresses: [],
      unresolved: [conflict],
    })
  })

  test("rejects invalid resolutions", () => {
    expect(() =>
      resolveConflicts([conflict], [{address: RAW_ZERO, source: "address-book", name: "Unknown"}]),
    ).toThrow("selects an unknown candidate")

    expect(() =>
      resolveConflicts([], [{address: RAW_ZERO, source: "address-book", name: "Beta"}]),
    ).toThrow("is stale")

    expect(() =>
      resolveConflicts(
        [conflict],
        [
          {address: RAW_ZERO, source: "ton-assets", name: "Alpha"},
          {address: RAW_ZERO, source: "address-book", name: "Beta"},
        ],
      ),
    ).toThrow("Duplicate conflict resolution")
  })
})

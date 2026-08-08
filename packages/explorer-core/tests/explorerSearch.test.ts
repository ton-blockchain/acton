import {describe, expect, test} from "bun:test"

import {parseBlockSearchQuery} from "../src/components/ExplorerSearch"

describe("parseBlockSearchQuery", () => {
  const expectedBlock = {
    workchain: -1,
    shard: "8000000000000000",
    seqno: 123_456,
  }

  test("accepts a full block ID without parentheses", () => {
    expect(parseBlockSearchQuery("-1,8000000000000000,123456")).toEqual(expectedBlock)
  })

  test("keeps accepting a full block ID in parentheses", () => {
    expect(parseBlockSearchQuery("(-1,8000000000000000,123456)")).toEqual(expectedBlock)
  })

  test("accepts a colon-separated full block ID", () => {
    expect(parseBlockSearchQuery("-1:8000000000000000:123456")).toEqual(expectedBlock)
  })

  test("accepts a parenthesized colon-separated full block ID", () => {
    expect(parseBlockSearchQuery("(-1:8000000000000000:123456)")).toEqual(expectedBlock)
  })

  test("rejects mixed separators", () => {
    expect(parseBlockSearchQuery("-1:8000000000000000,123456")).toBeUndefined()
    expect(parseBlockSearchQuery("-1,8000000000000000:123456")).toBeUndefined()
  })

  test("rejects unmatched parentheses", () => {
    expect(parseBlockSearchQuery("(-1:8000000000000000:123456")).toBeUndefined()
    expect(parseBlockSearchQuery("-1:8000000000000000:123456)")).toBeUndefined()
  })
})

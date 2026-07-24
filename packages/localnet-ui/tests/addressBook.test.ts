import {describe, expect, test} from "bun:test"

import {resolveAddressName} from "../src/explorer/hooks/useAddressBook"

describe("address name resolution", () => {
  test("prefers a custom name over ton-assets and TON DNS", () => {
    expect(resolveAddressName("My wallet", "Pavel Durov", "monk.t.me")).toBe("My wallet")
  })

  test("prefers ton-assets over TON DNS", () => {
    expect(resolveAddressName(undefined, "Pavel Durov", "monk.t.me")).toBe("Pavel Durov")
  })

  test("falls back to TON DNS", () => {
    expect(resolveAddressName(undefined, undefined, "monk.t.me")).toBe("monk.t.me")
  })
})

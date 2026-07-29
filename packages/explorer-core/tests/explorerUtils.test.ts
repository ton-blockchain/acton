import {describe, expect, test} from "bun:test"

import {
  formatAbsoluteTime,
  formatDnsName,
  formatTimeAgo,
  mergeAccountDomains,
  parseTonDnsSearchQuery,
  shortenIdentifier,
  toAccountQrAddress,
} from "../src/components/utils"

describe("toAccountQrAddress", () => {
  const rawAddress = "0:ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e"

  test("uses account-aware user-friendly URL-safe addresses", () => {
    const statuses = ["active", "frozen", "uninitialized", "uninit", "nonexist"] as const

    expect({
      mainnet: Object.fromEntries(
        statuses.map(status => [status, toAccountQrAddress(rawAddress, status, {testOnly: false})]),
      ),
      testnet: Object.fromEntries(
        statuses.map(status => [status, toAccountQrAddress(rawAddress, status, {testOnly: true})]),
      ),
    }).toMatchSnapshot()
  })
})

describe("parseTonDnsSearchQuery", () => {
  test("accepts .ton and .t.me TON DNS names", () => {
    expect(parseTonDnsSearchQuery("foundation.ton")).toBe("foundation.ton")
    expect(parseTonDnsSearchQuery("  MONK.T.ME  ")).toBe("monk.t.me")
  })

  test("rejects unsupported or malformed domains", () => {
    expect(parseTonDnsSearchQuery("monk.me")).toBeUndefined()
    expect(parseTonDnsSearchQuery("t.me/monk")).toBeUndefined()
    expect(parseTonDnsSearchQuery("-monk.t.me")).toBeUndefined()
  })
})

describe("formatDnsName", () => {
  test("decodes Punycode labels for display", () => {
    expect(formatDnsName("xn--037ha7bb.ton")).toBe("🅿🅰🅿🅰.ton")
  })

  test("keeps regular domain names unchanged", () => {
    expect(formatDnsName("monk.t.me")).toBe("monk.t.me")
  })
})

describe("mergeAccountDomains", () => {
  test("keeps the primary domain first and appends unique aliases", () => {
    expect(
      mergeAccountDomains("monk.t.me", ["wolf.t.me", "monk.t.me", "xn--037ha7bb.ton"]),
    ).toEqual(["monk.t.me", "wolf.t.me", "xn--037ha7bb.ton"])
  })

  test("trims names, ignores blanks, and deduplicates case-insensitively", () => {
    expect(
      mergeAccountDomains("  MONK.T.ME  ", ["monk.t.me", " ", " wolf.t.me ", "WOLF.T.ME"]),
    ).toEqual(["MONK.T.ME", "wolf.t.me"])
  })

  test("works without a primary domain", () => {
    expect(mergeAccountDomains(undefined, ["acton.ton", "mintmachine.ton"])).toEqual([
      "acton.ton",
      "mintmachine.ton",
    ])
  })
})

describe("shortenIdentifier", () => {
  test("keeps short identifiers unchanged", () => {
    expect(shortenIdentifier("123456789012")).toBe("123456789012")
  })

  test("preserves both ends of long identifiers", () => {
    expect(
      shortenIdentifier(
        "7971555897574548850977350810590246753707871758085628535730858724873159573504",
      ),
    ).toBe("797155…573504")
  })
})

describe("transaction dates", () => {
  const now = new Date(2026, 6, 20, 12, 0).getTime() / 1000

  test("omits the year for dates from the current year", () => {
    const timestamp = new Date(2026, 0, 15, 8, 30).getTime() / 1000

    expect(formatAbsoluteTime(timestamp, now)).toBe("15 Jan, 08:30")
    expect(formatTimeAgo(timestamp, now)).toBe("15 Jan, 08:30")
  })

  test("includes the year for dates from another year", () => {
    const timestamp = new Date(2025, 11, 31, 23, 45).getTime() / 1000

    expect(formatAbsoluteTime(timestamp, now)).toBe("31 Dec 2025, 23:45")
    expect(formatTimeAgo(timestamp, now)).toBe("31 Dec 2025, 23:45")
  })
})

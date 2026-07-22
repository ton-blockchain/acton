import {describe, expect, test} from "bun:test"

import {
  formatAbsoluteTime,
  formatTimeAgo,
  parseTonDnsSearchQuery,
  shortenIdentifier,
} from "../src/explorer/components/utils"

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

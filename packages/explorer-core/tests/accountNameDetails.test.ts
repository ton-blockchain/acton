import {describe, expect, test} from "bun:test"

import {getAccountNameDetails} from "../src/components/accountNameDetails"

describe("getAccountNameDetails", () => {
  test("separates TON DNS and Telegram names", () => {
    const details = getAccountNameDetails({
      displayName: "My wallet",
      customName: "My wallet",
      domains: ["acton.ton", "mintmachine.ton", "monk.t.me", "wolf.t.me"],
    })

    expect(details.groups).toEqual([
      {
        key: "ton-dns",
        label: "TON DNS",
        values: [
          {copyValue: "acton.ton", displayValue: "acton.ton"},
          {copyValue: "mintmachine.ton", displayValue: "mintmachine.ton"},
        ],
      },
      {
        key: "telegram",
        label: "Telegram",
        values: [
          {copyValue: "monk.t.me", displayValue: "monk.t.me"},
          {copyValue: "wolf.t.me", displayValue: "wolf.t.me"},
        ],
      },
    ])
  })

  test("excludes the currently displayed name from the details", () => {
    const details = getAccountNameDetails({
      displayName: "acton.ton",
      domain: "acton.ton",
      domains: ["acton.ton", "mintmachine.ton"],
    })

    expect(details.groups).toEqual([
      {
        key: "ton-dns",
        label: "TON DNS",
        values: [{copyValue: "mintmachine.ton", displayValue: "mintmachine.ton"}],
      },
    ])
  })

  test("excludes the currently displayed name case-insensitively", () => {
    const details = getAccountNameDetails({
      displayName: "MONK.T.ME",
      domain: "monk.t.me",
      domains: ["monk.t.me", "wolf.t.me"],
    })

    expect(details.groups).toEqual([
      {
        key: "telegram",
        label: "Telegram",
        values: [{copyValue: "wolf.t.me", displayValue: "wolf.t.me"}],
      },
    ])
  })

  test("deduplicates names from domain, domains, and tonDnsName", () => {
    const details = getAccountNameDetails({
      displayName: "My wallet",
      customName: "My wallet",
      domain: "acton.ton",
      domains: ["ACTON.TON", "mintmachine.ton"],
      tonDnsName: "MINTMACHINE.TON",
    })

    expect(details.groups).toEqual([
      {
        key: "ton-dns",
        label: "TON DNS",
        values: [
          {copyValue: "acton.ton", displayValue: "acton.ton"},
          {copyValue: "mintmachine.ton", displayValue: "mintmachine.ton"},
        ],
      },
    ])
  })

  test("returns no groups when there is no additional name data", () => {
    const details = getAccountNameDetails({
      displayName: "acton.ton",
      domain: "acton.ton",
    })

    expect(details.groups).toEqual([])
  })

  test("decodes Punycode for display and preserves the raw name for copying", () => {
    const details = getAccountNameDetails({
      displayName: "My wallet",
      customName: "My wallet",
      domain: "xn--037ha7bb.ton",
    })

    expect(details.groups).toEqual([
      {
        key: "ton-dns",
        label: "TON DNS",
        values: [{copyValue: "xn--037ha7bb.ton", displayValue: "🅿🅰🅿🅰.ton"}],
      },
    ])
  })
})

import {expect, test} from "bun:test"

import {TON_ASSETS_ACCOUNT_URLS, parseTonAssets} from "../scripts/sources/ton-assets.ts"

const RAW_ZERO = `0:${"0".repeat(64)}`
const BOUNCEABLE_ZERO = "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c"

test("parses ton-assets YAML", () => {
  expect(
    parseTonAssets(
      `
- address: "${BOUNCEABLE_ZERO}"
  name: "Alpha"
`,
      "example.yaml",
    ),
  ).toEqual([{address: RAW_ZERO, name: "Alpha"}])
})

test("does not apply address-book recovery to ton-assets", () => {
  expect(() =>
    parseTonAssets(
      `
- address: "${BOUNCEABLE_ZERO}"
  name: @wallet in Telegram
`,
      "example.yaml",
    ),
  ).toThrow("Failed to parse YAML from example.yaml")
})

test("excludes the upstream scammers file", () => {
  expect(TON_ASSETS_ACCOUNT_URLS.some(url => url.endsWith("/scammers.yaml"))).toBeFalse()
})

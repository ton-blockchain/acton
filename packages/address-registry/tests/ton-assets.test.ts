import {expect, test} from "bun:test"

import {
  TON_ASSETS_ACCOUNT_URLS,
  parseTonAssets,
  parseTonAssetsJettons,
} from "../scripts/sources/ton-assets.ts"

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
  ).toEqual([{address: BOUNCEABLE_ZERO, name: "Alpha"}])
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

test("parses the searchable fields from ton-assets jettons", () => {
  expect(
    parseTonAssetsJettons(
      JSON.stringify([
        {
          address: BOUNCEABLE_ZERO,
          description: "Ignored metadata",
          image: "https://example.com/alpha.png",
          name: "Alpha Token",
          symbol: "ALPHA",
        },
      ]),
      "jettons.json",
    ),
  ).toEqual([
    {
      address: `0:${"0".repeat(64)}`,
      image: "https://example.com/alpha.png",
      name: "Alpha Token",
      symbol: "ALPHA",
    },
  ])
})

test("rejects malformed ton-assets jettons", () => {
  expect(() =>
    parseTonAssetsJettons(
      JSON.stringify([{address: BOUNCEABLE_ZERO, name: "Alpha Token"}]),
      "jettons.json",
    ),
  ).toThrow("jettons.json[0].symbol must be a non-empty string")
})

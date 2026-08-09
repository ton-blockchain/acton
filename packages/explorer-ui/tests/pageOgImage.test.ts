import {describe, expect, test} from "bun:test"

import {pageOgPreviewForKey, pageOgPreviewForPath} from "../src/og/PageOgImage"

describe("pageOgPreviewForPath", () => {
  const cases = [
    ["/", "home"],
    ["/blocks", "blocks"],
    ["/blocks/", "blocks"],
    ["/abi", "abi"],
    ["/sources", "sources"],
    ["/faucet", "faucet"],
    ["/verified", "verified"],
    ["/verified/92bf1e3962a54b88", "verified-contract"],
    ["/cell", "cell"],
    ["/emulate", "emulate"],
    ["/favorites", "favorites"],
    ["/suspended", "suspended"],
    ["/block/-1/8000000000000000/123", "block"],
    ["/tx/abcdef", "transaction"],
    ["/tx/abcdef/trace", "transaction"],
  ] as const

  for (const [pathname, key] of cases) {
    test(`${pathname} uses the ${key} preview`, () => {
      expect(pageOgPreviewForPath(pathname)?.key).toBe(key)
    })
  }

  test("leaves account and ABI detail routes to their dynamic previews", () => {
    expect(pageOgPreviewForPath("/address/EQ123")).toBeUndefined()
    expect(pageOgPreviewForPath("/abi/wallet-v4")).toBeUndefined()
  })
})

test("unknown image keys fall back to the home preview", () => {
  expect(pageOgPreviewForKey("unknown").key).toBe("home")
})

test("favorites preview describes every supported favorite type", () => {
  expect(pageOgPreviewForKey("favorites")).toMatchObject({
    title: "Favorites",
    metadataTitle: "TON favorites · actonscan",
    metadataDescription: "Open your saved TON accounts and transactions on actonscan.",
  })
})

test("faucet preview makes the Testnet scope explicit", () => {
  expect(pageOgPreviewForKey("faucet")).toMatchObject({
    title: "Testnet Faucet",
    metadataTitle: "TON Testnet Faucet · actonscan",
  })
})

test("suspended addresses use dedicated social metadata", () => {
  expect(pageOgPreviewForKey("suspended")).toMatchObject({
    title: "Suspended addresses",
    metadataTitle: "Suspended TON addresses · actonscan",
    metadataDescription:
      "Browse TON addresses suspended through validators' voting and check when restrictions expire on actonscan.",
  })
})

test("verified routes use dedicated social metadata", () => {
  expect(pageOgPreviewForKey("verified")).toMatchObject({
    title: "Verified contracts",
    metadataTitle: "Verified TON contracts · actonscan",
  })
  expect(pageOgPreviewForKey("verified-contract")).toMatchObject({
    title: "Verified contract",
    metadataTitle: "Verified TON contract · actonscan",
  })
})

import {describe, expect, test} from "bun:test"

import {pageOgPreviewForKey, pageOgPreviewForPath} from "../src/og/PageOgImage"

describe("pageOgPreviewForPath", () => {
  const cases = [
    ["/", "home"],
    ["/blocks", "blocks"],
    ["/blocks/", "blocks"],
    ["/abi", "abi"],
    ["/sources", "sources"],
    ["/cell", "cell"],
    ["/emulate", "emulate"],
    ["/favorites", "favorites"],
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

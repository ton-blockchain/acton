import {expect, test} from "bun:test"
import {sha256_sync} from "@ton/crypto"

import {createNftSafetyMatcher, NSFW_NFT_REGISTRY} from "../src/explorer/nftSafetyRegistry"

const hashRegistryValue = (value: string): string => sha256_sync(value).toString("hex")

const contentHash = "a".repeat(64)
const isRegisteredNsfwNft = createNftSafetyMatcher({
  imageUrlHashes: [hashRegistryValue("https://images.example/blocked.png")],
  imageHostSuffixHashes: [hashRegistryValue("blocked.example")],
  contentHashes: [contentHash],
  collectionNameHashes: [hashRegistryValue("blocked collection")],
})

const toncenterProxyUrl = (source: string): string => {
  const encodedSource = btoa(source).replace(/\+/g, "-").replace(/\//g, "_").replace(/[=]+$/, "")
  return `https://proxy.toncenter.com/proxy-id/pr:small/${encodedSource}`
}

test("stores only anonymized registry values", () => {
  expect(
    Object.values(NSFW_NFT_REGISTRY)
      .flat()
      .every(value => /^[0-9a-f]{64}$/.test(value)),
  ).toBe(true)
})

test("matches manually registered NFT content before and after downloading it", () => {
  expect({
    urlWithChangingQuery: isRegisteredNsfwNft({
      imageUrl: "https://images.example/blocked.png?t=changing-value",
    }),
    randomizedSubdomain: isRegisteredNsfwNft({
      imageUrl: "https://new-random-name.blocked.example/images/other",
    }),
    exactUrlWithChangingQuery: isRegisteredNsfwNft({
      imageUrl: "https://images.example/blocked.png?t=new-value",
    }),
    normalizedCollectionName: isRegisteredNsfwNft({
      collectionName: "  BLOCKED   collection ",
    }),
    contentHash: isRegisteredNsfwNft({
      contentHash: `sha256:${contentHash.toUpperCase()}`,
    }),
    toncenterProxyWithCanonicalHash: isRegisteredNsfwNft({
      imageUrl: toncenterProxyUrl(`local:///sha256/${contentHash}`),
    }),
    toncenterProxyWithRegisteredSourceUrl: isRegisteredNsfwNft({
      imageUrl: toncenterProxyUrl("https://cdn.blocked.example/images/other?t=changing-value"),
    }),
    unknown: isRegisteredNsfwNft({
      imageUrl: "https://example.com/safe.png",
      collectionName: "Example collection",
      contentHash: "0000000000000000000000000000000000000000000000000000000000000000",
    }),
  }).toMatchInlineSnapshot(`
    {
      "contentHash": true,
      "exactUrlWithChangingQuery": true,
      "normalizedCollectionName": true,
      "randomizedSubdomain": true,
      "toncenterProxyWithCanonicalHash": true,
      "toncenterProxyWithRegisteredSourceUrl": true,
      "unknown": false,
      "urlWithChangingQuery": true,
    }
  `)
})

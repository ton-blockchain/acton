import {expect, test} from "bun:test"

import {isRegisteredNsfwNft} from "../src/explorer/nftSafetyRegistry"

test("matches manually registered NFT content before and after downloading it", () => {
  expect({
    urlWithChangingQuery: isRegisteredNsfwNft({
      imageUrl: "https://cloudmetrics.cyou/images/3?t=1783902740484",
    }),
    randomizedSubdomain: isRegisteredNsfwNft({
      imageUrl: "https://new-random-name.cloudmetrics.cyou/images/other",
    }),
    proxiedUrlWithChangingQuery: isRegisteredNsfwNft({
      imageUrl:
        "https://cache.tonapi.io/imgproxy/wDYWflKVCrSqqoVsEEXrJWTgPzfxJZUbygFjXcqdOcc/rs:fill:1500:1500:1/g:no/aHR0cHM6Ly80NjI5LmNsb3VkbWV0cmljcy5jeW91L2ltYWdlcy8zP3Q9MTc4MzYyNzgyMzIzNw.webp?t=new-value",
    }),
    normalizedCollectionName: isRegisteredNsfwNft({
      collectionName: "  BUNKER   of Death 1781768564560 ",
    }),
    contentHash: isRegisteredNsfwNft({
      contentHash: "sha256:EAD9E3C5F260785E8852C0DCA7AD1FD7E6690B03009A158A849243E59685AA7D",
    }),
    toncenterProxyWithCanonicalHash: isRegisteredNsfwNft({
      imageUrl:
        "https://proxy.toncenter.com/F0W0fr2CnSPVMdgFNe9x87X1TkFGKz7rUBtHpWmNXwc/pr:small/bG9jYWw6Ly8vc2hhMjU2L2VhZDllM2M1ZjI2MDc4NWU4ODUyYzBkY2E3YWQxZmQ3ZTY2OTBiMDMwMDlhMTU4YTg0OTI0M2U1OTY4NWFhN2Q",
    }),
    toncenterProxyWithRegisteredSourceUrl: isRegisteredNsfwNft({
      imageUrl:
        "https://imgproxy.toncenter.com/hD7GRJtQMYSy89vvq6xqb-tVtbtBhpdE2S7xjNYiqcw/pr:small/aHR0cHM6Ly9tenQzLmNsb3VkbWV0cmljcy5jeW91L2ltYWdlcy8zP3Q9MTc4MTkwNTYxOTE4MQ",
    }),
    unknown: isRegisteredNsfwNft({
      imageUrl: "https://example.com/safe.png",
      collectionName: "Example collection",
      contentHash: "0000000000000000000000000000000000000000000000000000000000000000",
    }),
  }).toMatchInlineSnapshot(`
    {
      "contentHash": true,
      "normalizedCollectionName": true,
      "proxiedUrlWithChangingQuery": true,
      "randomizedSubdomain": true,
      "toncenterProxyWithCanonicalHash": true,
      "toncenterProxyWithRegisteredSourceUrl": true,
      "unknown": false,
      "urlWithChangingQuery": true,
    }
  `)
})

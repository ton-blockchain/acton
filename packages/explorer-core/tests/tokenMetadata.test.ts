import {describe, expect, test} from "bun:test"

import {metadataTokenDecimals} from "../src/api/tokenMetadata"

describe("metadataTokenDecimals", () => {
  test("reads supported decimal metadata forms", () => {
    expect({
      directNumber: metadataTokenDecimals({type: "jetton_masters", decimals: 9}),
      directString: metadataTokenDecimals({type: "jetton_masters", decimals: "6"}),
      extraNumber: metadataTokenDecimals({
        type: "jetton_masters",
        extra: {decimals: 9},
      }),
      invalid: metadataTokenDecimals({type: "jetton_masters", decimals: 37}),
    }).toMatchInlineSnapshot(`
      {
        "directNumber": 9,
        "directString": 6,
        "extraNumber": 9,
        "invalid": undefined,
      }
    `)
  })
})

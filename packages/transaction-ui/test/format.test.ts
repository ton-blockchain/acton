import {describe, expect, test} from "bun:test"

import {shortenMiddle} from "@acton/ui"

describe("transaction UI formatting", () => {
  test("truncates long labels in the middle", () => {
    expect([
      shortenMiddle("short-name.ton", {maxLength: 20}),
      shortenMiddle("blackmarket-dot-tg-exch.ton", {maxLength: 20}),
    ]).toMatchInlineSnapshot(`
      [
        "short-name.ton",
        "blackmarke…-exch.ton",
      ]
    `)
  })
})

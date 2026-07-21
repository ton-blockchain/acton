import {describe, expect, test} from "bun:test"

import {truncateMiddle} from "../src/lib/format"

describe("transaction UI formatting", () => {
  test("truncates long labels in the middle", () => {
    expect([
      truncateMiddle("short-name.ton", 20),
      truncateMiddle("blackmarket-dot-tg-exch.ton", 20),
    ]).toMatchInlineSnapshot(`
      [
        "short-name.ton",
        "blackmarke…-exch.ton",
      ]
    `)
  })
})

import {describe, expect, test} from "bun:test"

import {parsePageSearchParam, withPageSearchParam} from "../src/hooks/useSearchParamPagination"

describe("search parameter pagination", () => {
  test("parses positive integer pages and rejects invalid values", () => {
    expect(
      [null, "", "0", "-2", "1.5", "abc", "1", "002", String(Number.MAX_SAFE_INTEGER + 1)].map(
        parsePageSearchParam,
      ),
    ).toMatchInlineSnapshot(`
      [
        1,
        1,
        1,
        1,
        1,
        1,
        1,
        2,
        1,
      ]
    `)
  })

  test("updates only the page parameter and omits the first page", () => {
    const original = new URLSearchParams("network=testnet&page=4&view=compact")
    const secondPage = withPageSearchParam(original, 2)
    const firstPage = withPageSearchParam(secondPage, 1)

    expect({
      original: original.toString(),
      secondPage: secondPage.toString(),
      firstPage: firstPage.toString(),
    }).toMatchInlineSnapshot(`
      {
        "firstPage": "network=testnet&view=compact",
        "original": "network=testnet&page=4&view=compact",
        "secondPage": "network=testnet&page=2&view=compact",
      }
    `)
  })
})

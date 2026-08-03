import {describe, expect, test} from "bun:test"

import {
  DEFAULT_PAGE_SIZE,
  getPaginationItems,
  getPaginationState,
} from "../src/components/Pagination"

describe("pagination", () => {
  test("normalizes page bounds and item ranges", () => {
    expect({
      empty: getPaginationState(0, 8),
      first: getPaginationState(212, 1),
      middle: getPaginationState(212, 6),
      last: getPaginationState(212, 99),
      invalid: getPaginationState(Number.NaN, Number.NaN, 0),
    }).toMatchInlineSnapshot(`
      {
        "empty": {
          "currentPage": 1,
          "endIndex": 0,
          "pageSize": 20,
          "startIndex": 0,
          "totalItems": 0,
          "totalPages": 1,
        },
        "first": {
          "currentPage": 1,
          "endIndex": 20,
          "pageSize": 20,
          "startIndex": 0,
          "totalItems": 212,
          "totalPages": 11,
        },
        "invalid": {
          "currentPage": 1,
          "endIndex": 0,
          "pageSize": 1,
          "startIndex": 0,
          "totalItems": 0,
          "totalPages": 1,
        },
        "last": {
          "currentPage": 11,
          "endIndex": 212,
          "pageSize": 20,
          "startIndex": 200,
          "totalItems": 212,
          "totalPages": 11,
        },
        "middle": {
          "currentPage": 6,
          "endIndex": 120,
          "pageSize": 20,
          "startIndex": 100,
          "totalItems": 212,
          "totalPages": 11,
        },
      }
    `)
    expect(DEFAULT_PAGE_SIZE).toBe(20)
  })

  test("keeps the current page visible in compact page controls", () => {
    expect({
      short: getPaginationItems(3, 5),
      beginning: getPaginationItems(1, 11),
      middle: getPaginationItems(6, 11),
      end: getPaginationItems(11, 11),
    }).toMatchInlineSnapshot(`
      {
        "beginning": [
          1,
          2,
          3,
          4,
          5,
          "ellipsis-right",
          11,
        ],
        "end": [
          1,
          "ellipsis-left",
          7,
          8,
          9,
          10,
          11,
        ],
        "middle": [
          1,
          "ellipsis-left",
          5,
          6,
          7,
          "ellipsis-right",
          11,
        ],
        "short": [
          1,
          2,
          3,
          4,
          5,
        ],
      }
    `)
  })
})

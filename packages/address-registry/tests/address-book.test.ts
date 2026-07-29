import {expect, test} from "bun:test"

import {ADDRESS_BOOK_URLS, parseAddressBook} from "../scripts/sources/address-book.ts"

const RAW_ZERO = `0:${"0".repeat(64)}`
const RAW_ONES = `0:${"1".repeat(64)}`
const BOUNCEABLE_ZERO = "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c"

test("matches upstream recovery for reserved plain name scalars", () => {
  expect(
    parseAddressBook(
      `
- address: "${BOUNCEABLE_ZERO}"
  name: @wallet in Telegram
`,
      "example.yaml",
    ),
  ).toEqual([{address: RAW_ZERO, name: "@wallet in Telegram"}])
})

test("matches upstream convention for disabled indented entries", () => {
  expect(
    parseAddressBook(
      `
- address: "${BOUNCEABLE_ZERO}"
  name: "Active"

  - address: "${RAW_ONES}"
  name: "Disabled"
  type: wallet
`,
      "example.yaml",
    ),
  ).toEqual([{address: RAW_ZERO, name: "Active"}])
})

test("excludes the upstream scam file", () => {
  expect(ADDRESS_BOOK_URLS.some(url => url.endsWith("/scam.yaml"))).toBeFalse()
})

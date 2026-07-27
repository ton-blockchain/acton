import {describe, expect, test} from "bun:test"

import {readSources} from "../scripts/sources.ts"
import {ADDRESS_BOOK_URLS, parseAddressBook} from "../scripts/sources/address-book.ts"
import {parseSourceAddresses} from "../scripts/sources/shared.ts"
import {TON_ASSETS_ACCOUNT_URLS, parseTonAssets} from "../scripts/sources/ton-assets.ts"

const RAW_ZERO = `0:${"0".repeat(64)}`
const RAW_ONES = `0:${"1".repeat(64)}`
const BOUNCEABLE_ZERO = "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c"
const NON_BOUNCEABLE_ZERO = "UQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAJKZ"

describe("source address validation", () => {
  test("normalizes addresses and reads names from every entry", () => {
    expect(
      parseSourceAddresses(
        [
          {address: BOUNCEABLE_ZERO, name: "Alpha"},
          {address: RAW_ONES, name: "Beta", extra: true},
        ],
        "example.yaml",
      ),
    ).toEqual([
      {address: RAW_ZERO, name: "Alpha"},
      {address: RAW_ONES, name: "Beta"},
    ])
  })

  test("normalizes friendly variants to the same raw address", () => {
    const addresses = parseSourceAddresses(
      [
        {address: BOUNCEABLE_ZERO, name: "Bounceable"},
        {address: NON_BOUNCEABLE_ZERO, name: "Non-bounceable"},
      ],
      "example.yaml",
    )

    expect(addresses.map(({address}) => address)).toEqual([RAW_ZERO, RAW_ZERO])
  })

  test("rejects malformed entries", () => {
    expect(() => parseSourceAddresses({}, "example.yaml")).toThrow("must contain an array")
    expect(() => parseSourceAddresses([{address: RAW_ZERO}], "example.yaml")).toThrow(
      "example.yaml[0].name",
    )
    expect(() =>
      parseSourceAddresses([{address: "not-an-address", name: "Invalid"}], "example.yaml"),
    ).toThrow("example.yaml[0].address must be a valid TON address")
  })
})

describe("source YAML parsing", () => {
  test("parses YAML before validating entries", () => {
    expect(
      parseTonAssets(
        `
- address: "${BOUNCEABLE_ZERO}"
  name: "Alpha"
`,
        "example.yaml",
      ),
    ).toEqual([{address: RAW_ZERO, name: "Alpha"}])
  })

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

  test("does not apply address-book recovery to ton-assets", () => {
    expect(() =>
      parseTonAssets(
        `
- address: "${BOUNCEABLE_ZERO}"
  name: @wallet in Telegram
`,
        "example.yaml",
      ),
    ).toThrow("Failed to parse YAML from example.yaml")
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
})

test("source lists exclude upstream scam files", () => {
  expect(TON_ASSETS_ACCOUNT_URLS.some(url => url.endsWith("/scammers.yaml"))).toBeFalse()
  expect(ADDRESS_BOOK_URLS.some(url => url.endsWith("/scam.yaml"))).toBeFalse()
})

test("readSources reads every allowed upstream YAML file", async () => {
  const requestedUrls: string[] = []
  const read = (url: string): Promise<string> => {
    requestedUrls.push(url)
    return Promise.resolve(`
- address: "${BOUNCEABLE_ZERO}"
  name: "${url}"
`)
  }

  const sources = await readSources(read)

  expect(requestedUrls).toEqual([...TON_ASSETS_ACCOUNT_URLS, ...ADDRESS_BOOK_URLS])
  expect(sources).toEqual([
    {
      id: "ton-assets",
      urls: TON_ASSETS_ACCOUNT_URLS,
      addresses: TON_ASSETS_ACCOUNT_URLS.map(name => ({address: RAW_ZERO, name})),
    },
    {
      id: "address-book",
      urls: ADDRESS_BOOK_URLS,
      addresses: ADDRESS_BOOK_URLS.map(name => ({address: RAW_ZERO, name})),
    },
  ])
})

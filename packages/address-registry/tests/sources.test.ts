import {describe, expect, test} from "bun:test"

import {readSources} from "../scripts/sources.ts"
import {ADDRESS_BOOK_URLS, parseAddressBook} from "../scripts/sources/address-book.ts"
import {parseSourceAddresses} from "../scripts/sources/shared.ts"
import {TON_ASSETS_ACCOUNT_URLS, parseTonAssets} from "../scripts/sources/ton-assets.ts"

describe("source parsing", () => {
  test("reads address and name from every entry", () => {
    expect(
      parseSourceAddresses(
        [
          {address: "0:abc", name: "Alpha"},
          {address: "EQ123", name: "Beta", extra: true},
        ],
        "example.yaml",
      ),
    ).toEqual([
      {address: "0:abc", name: "Alpha"},
      {address: "EQ123", name: "Beta"},
    ])
  })

  test("parses YAML before validating entries", () => {
    expect(
      parseTonAssets(
        `
- address: "0:abc"
  name: "Alpha"
`,
        "example.yaml",
      ),
    ).toEqual([{address: "0:abc", name: "Alpha"}])
  })

  test("matches upstream recovery for reserved plain name scalars", () => {
    expect(
      parseAddressBook(
        `
- address: "0:abc"
  name: @wallet in Telegram
`,
        "example.yaml",
      ),
    ).toEqual([{address: "0:abc", name: "@wallet in Telegram"}])
  })

  test("does not apply address-book recovery to ton-assets", () => {
    expect(() =>
      parseTonAssets(
        `
- address: "0:abc"
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
- address: "0:active"
  name: "Active"

  - address: "0:disabled"
  name: "Disabled"
  type: wallet
`,
        "example.yaml",
      ),
    ).toEqual([{address: "0:active", name: "Active"}])
  })

  test("rejects malformed entries", () => {
    expect(() => parseSourceAddresses({}, "example.yaml")).toThrow("must contain an array")
    expect(() => parseSourceAddresses([{address: "0:abc"}], "example.yaml")).toThrow(
      "example.yaml[0].name",
    )
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
- address: "${url}"
  name: "Example"
`)
  }

  const sources = await readSources(read)

  expect(requestedUrls).toEqual([...TON_ASSETS_ACCOUNT_URLS, ...ADDRESS_BOOK_URLS])
  expect(sources).toEqual([
    {
      id: "ton-assets",
      urls: TON_ASSETS_ACCOUNT_URLS,
      addresses: TON_ASSETS_ACCOUNT_URLS.map(address => ({address, name: "Example"})),
    },
    {
      id: "address-book",
      urls: ADDRESS_BOOK_URLS,
      addresses: ADDRESS_BOOK_URLS.map(address => ({address, name: "Example"})),
    },
  ])
})

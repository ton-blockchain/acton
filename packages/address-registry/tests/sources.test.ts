import {describe, expect, test} from "bun:test"

import {
  ADDRESS_BOOK_URL,
  TON_ASSETS_ACCOUNTS_URL,
  parseAddressBook,
  parseTonAssetsAccounts,
  readSources,
} from "../scripts/sources.ts"

describe("parseTonAssetsAccounts", () => {
  test("reads address and name from every array entry", () => {
    expect(
      parseTonAssetsAccounts([
        {address: "0:abc", name: "Alpha"},
        {address: "EQ123", name: "Beta", extra: true},
      ]),
    ).toEqual([
      {address: "0:abc", name: "Alpha"},
      {address: "EQ123", name: "Beta"},
    ])
  })

  test("rejects malformed entries", () => {
    expect(() => parseTonAssetsAccounts({})).toThrow("must be an array")
    expect(() => parseTonAssetsAccounts([{address: "0:abc"}])).toThrow("accounts.json[0].name")
  })
})

describe("parseAddressBook", () => {
  test("reads addresses from object keys and optional names from metadata", () => {
    const alphaAddress = "EQ123"
    const betaAddress = "UQ456"
    const scamAddress = "UQ789"

    expect(
      parseAddressBook({
        [alphaAddress]: {name: "Alpha"},
        [betaAddress]: {name: "Beta", tonIcon: "💎"},
        [scamAddress]: {isScam: true},
      }),
    ).toEqual([
      {address: alphaAddress, name: "Alpha"},
      {address: betaAddress, name: "Beta"},
      {address: scamAddress},
    ])
  })

  test("rejects malformed metadata", () => {
    const address = "EQ123"

    expect(() => parseAddressBook([])).toThrow("must be an object")
    expect(() => parseAddressBook({[address]: {name: null}})).toThrow(
      `addresses.json[${JSON.stringify(address)}].name`,
    )
  })
})

test("readSources reads both upstream URLs", async () => {
  const requestedUrls: string[] = []
  const read = (url: string): Promise<unknown> => {
    requestedUrls.push(url)

    if (url === TON_ASSETS_ACCOUNTS_URL) {
      return Promise.resolve([{address: "0:abc", name: "Ton Assets"}])
    }
    if (url === ADDRESS_BOOK_URL) {
      const address = "EQ123"
      return Promise.resolve({[address]: {name: "Address Book"}})
    }

    return Promise.reject(new Error(`Unexpected URL: ${url}`))
  }

  const sources = await readSources(read)

  expect(requestedUrls).toEqual([TON_ASSETS_ACCOUNTS_URL, ADDRESS_BOOK_URL])
  expect(sources).toEqual([
    {
      id: "ton-assets",
      url: TON_ASSETS_ACCOUNTS_URL,
      addresses: [{address: "0:abc", name: "Ton Assets"}],
    },
    {
      id: "address-book",
      url: ADDRESS_BOOK_URL,
      addresses: [{address: "EQ123", name: "Address Book"}],
    },
  ])
})

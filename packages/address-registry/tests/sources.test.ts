import {describe, expect, test} from "bun:test"

import {readSources} from "../scripts/sources.ts"
import {ACTON_ADDRESSES} from "../scripts/sources/acton.ts"
import {ADDRESS_BOOK_URLS} from "../scripts/sources/address-book.ts"
import {parseSourceAddresses} from "../scripts/sources/shared.ts"
import {TON_ASSETS_ACCOUNT_URLS} from "../scripts/sources/ton-assets.ts"

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

test("readSources reads every allowed upstream YAML file and the Acton list", async () => {
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
    {
      id: "acton",
      urls: [],
      addresses: parseSourceAddresses(ACTON_ADDRESSES, "ACTON_ADDRESSES"),
    },
  ])
})

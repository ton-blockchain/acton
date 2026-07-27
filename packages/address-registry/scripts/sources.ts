export const TON_ASSETS_ACCOUNTS_URL =
  "https://raw.githubusercontent.com/tonkeeper/ton-assets/main/accounts.json"

export const ADDRESS_BOOK_URL = "https://address-book.tonscan.org/addresses.json"

export type AddressSourceId = "ton-assets" | "address-book"

export interface SourceAddress {
  readonly address: string
  readonly name?: string
}

export interface AddressSource {
  readonly id: AddressSourceId
  readonly url: string
  readonly addresses: readonly SourceAddress[]
}

type JsonReader = (url: string) => Promise<unknown>

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const requireNonEmptyString = (value: unknown, path: string): string => {
  if (typeof value !== "string" || value.length === 0) {
    throw new TypeError(`${path} must be a non-empty string`)
  }

  return value
}

export const parseTonAssetsAccounts = (value: unknown): readonly SourceAddress[] => {
  if (!Array.isArray(value)) {
    throw new TypeError("ton-assets accounts.json must be an array")
  }

  return value.map((row, index) => {
    const path = `ton-assets accounts.json[${index}]`
    if (!isRecord(row)) {
      throw new TypeError(`${path} must be an object`)
    }

    return {
      address: requireNonEmptyString(row.address, `${path}.address`),
      name: requireNonEmptyString(row.name, `${path}.name`),
    }
  })
}

export const parseAddressBook = (value: unknown): readonly SourceAddress[] => {
  if (!isRecord(value)) {
    throw new TypeError("address-book addresses.json must be an object")
  }

  return Object.entries(value).map(([address, metadata]) => {
    const path = `address-book addresses.json[${JSON.stringify(address)}]`
    if (!isRecord(metadata)) {
      throw new TypeError(`${path} must be an object`)
    }

    const sourceAddress: SourceAddress = {
      address: requireNonEmptyString(address, `${path} key`),
    }

    if (metadata.name === undefined) {
      return sourceAddress
    }

    return {
      ...sourceAddress,
      name: requireNonEmptyString(metadata.name, `${path}.name`),
    }
  })
}

export const readJson = async (url: string): Promise<unknown> => {
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(`Failed to read ${url}: HTTP ${response.status} ${response.statusText}`)
  }

  try {
    return await response.json()
  } catch (error) {
    throw new SyntaxError(`Failed to parse JSON from ${url}`, {cause: error})
  }
}

export const readTonAssets = async (read: JsonReader = readJson): Promise<AddressSource> => ({
  id: "ton-assets",
  url: TON_ASSETS_ACCOUNTS_URL,
  addresses: parseTonAssetsAccounts(await read(TON_ASSETS_ACCOUNTS_URL)),
})

export const readAddressBook = async (read: JsonReader = readJson): Promise<AddressSource> => ({
  id: "address-book",
  url: ADDRESS_BOOK_URL,
  addresses: parseAddressBook(await read(ADDRESS_BOOK_URL)),
})

export const readSources = async (
  read: JsonReader = readJson,
): Promise<readonly [AddressSource, AddressSource]> =>
  Promise.all([readTonAssets(read), readAddressBook(read)])

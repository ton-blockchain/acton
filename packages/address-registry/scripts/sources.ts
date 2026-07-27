import {parseAllDocuments} from "yaml"

const TON_ASSETS_ACCOUNTS_BASE_URL =
  "https://raw.githubusercontent.com/tonkeeper/ton-assets/main/accounts"
const ADDRESS_BOOK_BASE_URL =
  "https://raw.githubusercontent.com/catchain/address-book/master/source"

const TON_ASSETS_ACCOUNT_FILES = [
  "bridges.yaml",
  "celebrities.yaml",
  "custodians.yaml",
  "dapps.yaml",
  "defi.yaml",
  "givers.yaml",
  "infrastructure.yaml",
  "notcoin.yaml",
  "ston.yaml",
  "validators.yaml",
] as const

const ADDRESS_BOOK_FILES = [
  "community.yaml",
  "exchanges.yaml",
  "people.yaml",
  "system.yaml",
  "validators.yaml",
] as const

export const TON_ASSETS_ACCOUNT_URLS = TON_ASSETS_ACCOUNT_FILES.map(
  fileName => `${TON_ASSETS_ACCOUNTS_BASE_URL}/${fileName}`,
)

export const ADDRESS_BOOK_URLS = ADDRESS_BOOK_FILES.map(
  fileName => `${ADDRESS_BOOK_BASE_URL}/${fileName}`,
)

export type AddressSourceId = "ton-assets" | "address-book"

export interface SourceAddress {
  readonly address: string
  readonly name: string
}

export interface AddressSource {
  readonly id: AddressSourceId
  readonly urls: readonly string[]
  readonly addresses: readonly SourceAddress[]
}

type TextReader = (url: string) => Promise<string>

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const requireNonEmptyString = (value: unknown, path: string): string => {
  if (typeof value !== "string" || value.length === 0) {
    throw new TypeError(`${path} must be a non-empty string`)
  }

  return value
}

export const parseSourceAddresses = (
  value: unknown,
  sourcePath: string,
): readonly SourceAddress[] => {
  if (!Array.isArray(value)) {
    throw new TypeError(`${sourcePath} must contain an array`)
  }

  return value.map((row, index) => {
    const path = `${sourcePath}[${index}]`
    if (!isRecord(row)) {
      throw new TypeError(`${path} must be an object`)
    }

    return {
      address: requireNonEmptyString(row.address, `${path}.address`),
      name: requireNonEmptyString(row.name, `${path}.name`),
    }
  })
}

export const parseYamlAddresses = (text: string, sourcePath: string): readonly SourceAddress[] => {
  try {
    const entries = parseAllDocuments(text, {logLevel: "silent"}).flatMap(document =>
      document.toJS(),
    )
    return parseSourceAddresses(entries, sourcePath)
  } catch (error) {
    if (error instanceof TypeError) {
      throw error
    }

    throw new SyntaxError(`Failed to parse YAML from ${sourcePath}`, {cause: error})
  }
}

export const readText = async (url: string): Promise<string> => {
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(`Failed to read ${url}: HTTP ${response.status} ${response.statusText}`)
  }

  return response.text()
}

const readSource = async (
  id: AddressSourceId,
  urls: readonly string[],
  read: TextReader,
): Promise<AddressSource> => {
  const files = await Promise.all(urls.map(async url => parseYamlAddresses(await read(url), url)))

  return {
    id,
    urls,
    addresses: files.flat(),
  }
}

export const readTonAssets = async (read: TextReader = readText): Promise<AddressSource> =>
  readSource("ton-assets", TON_ASSETS_ACCOUNT_URLS, read)

export const readAddressBook = async (read: TextReader = readText): Promise<AddressSource> =>
  readSource("address-book", ADDRESS_BOOK_URLS, read)

export const readSources = async (
  read: TextReader = readText,
): Promise<readonly [AddressSource, AddressSource]> =>
  Promise.all([readTonAssets(read), readAddressBook(read)])

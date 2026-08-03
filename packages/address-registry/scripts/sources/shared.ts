import {Address} from "@ton/core"

export type AddressSourceId = "ton-assets" | "address-book" | "acton"

export interface SourceAddress {
  readonly address: string
  readonly name: string
}

export interface AddressSource {
  readonly id: AddressSourceId
  readonly urls: readonly string[]
  readonly addresses: readonly SourceAddress[]
}

export type TextReader = (url: string) => Promise<string>

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const requireNonEmptyString = (value: unknown, path: string): string => {
  if (typeof value !== "string" || value.length === 0) {
    throw new TypeError(`${path} must be a non-empty string`)
  }

  return value
}

const requireAddress = (value: unknown, path: string): string => {
  const source = requireNonEmptyString(value, path)

  try {
    Address.parse(source)
    return source
  } catch (error) {
    throw new TypeError(`${path} must be a valid TON address`, {cause: error})
  }
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
      address: requireAddress(row.address, `${path}.address`),
      name: requireNonEmptyString(row.name, `${path}.name`),
    }
  })
}

export const readText = async (url: string): Promise<string> => {
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(`Failed to read ${url}: HTTP ${response.status} ${response.statusText}`)
  }

  return response.text()
}

export const readSource = async (
  id: AddressSourceId,
  urls: readonly string[],
  parse: (text: string, sourcePath: string) => readonly SourceAddress[],
  read: TextReader,
): Promise<AddressSource> => {
  const files = await Promise.all(urls.map(async url => parse(await read(url), url)))

  return {
    id,
    urls,
    addresses: files.flat(),
  }
}

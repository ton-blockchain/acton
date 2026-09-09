import {Address} from "@ton/core"
import {parse} from "yaml"

import type {AddressSource, TextReader} from "./shared.ts"
import {parseSourceAddresses, readSource, readText} from "./shared.ts"

const TON_ASSETS_ACCOUNTS_BASE_URL =
  "https://raw.githubusercontent.com/tonkeeper/ton-assets/main/accounts"

export const TON_ASSETS_JETTONS_URL =
  "https://raw.githubusercontent.com/tonkeeper/ton-assets/main/jettons.json"

const TON_ASSETS_ACCOUNT_FILES = [
  "bridges",
  "celebrities",
  "custodians",
  "dapps",
  "defi",
  "givers",
  "infrastructure",
  "notcoin",
  "ston",
  "validators",
] as const

export const TON_ASSETS_ACCOUNT_URLS = TON_ASSETS_ACCOUNT_FILES.map(
  fileName => `${TON_ASSETS_ACCOUNTS_BASE_URL}/${fileName}.yaml`,
)

export const parseTonAssets = (text: string, sourcePath: string) => {
  try {
    return parseSourceAddresses(parse(text), sourcePath)
  } catch (error) {
    if (error instanceof TypeError) {
      throw error
    }

    throw new SyntaxError(`Failed to parse YAML from ${sourcePath}`, {cause: error})
  }
}

export interface TonAssetsJetton {
  readonly address: string
  readonly image?: string
  readonly name: string
  readonly symbol: string
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const requireString = (value: unknown, path: string): string => {
  if (typeof value !== "string" || value.length === 0) {
    throw new TypeError(`${path} must be a non-empty string`)
  }

  return value
}

/**
 * Validates the generated ton-assets jetton catalog before it is bundled into the registry.
 * The parser intentionally keeps only fields used by search so upstream metadata cannot grow
 * the Explorer bundle unnoticed.
 */
export const parseTonAssetsJettons = (
  text: string,
  sourcePath: string,
): readonly TonAssetsJetton[] => {
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch (error) {
    throw new SyntaxError(`Failed to parse JSON from ${sourcePath}`, {cause: error})
  }

  if (!Array.isArray(value)) {
    throw new TypeError(`${sourcePath} must contain an array`)
  }

  return value.map((row, index) => {
    const path = `${sourcePath}[${index}]`
    if (!isRecord(row)) {
      throw new TypeError(`${path} must be an object`)
    }

    const sourceAddress = requireString(row.address, `${path}.address`)
    let address: string
    try {
      address = Address.parse(sourceAddress).toRawString()
    } catch (error) {
      throw new TypeError(`${path}.address must be a valid TON address`, {cause: error})
    }

    const image = row.image === undefined ? undefined : requireString(row.image, `${path}.image`)

    return {
      address,
      ...(image ? {image} : {}),
      name: requireString(row.name, `${path}.name`),
      symbol: requireString(row.symbol, `${path}.symbol`),
    }
  })
}

export const readTonAssets = async (read: TextReader = readText): Promise<AddressSource> =>
  readSource("ton-assets", TON_ASSETS_ACCOUNT_URLS, parseTonAssets, read)

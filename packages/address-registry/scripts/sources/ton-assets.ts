import {parse} from "yaml"

import type {AddressSource, TextReader} from "./shared.ts"
import {parseSourceAddresses, readSource, readText} from "./shared.ts"

const TON_ASSETS_ACCOUNTS_BASE_URL =
  "https://raw.githubusercontent.com/tonkeeper/ton-assets/main/accounts"

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

export const TON_ASSETS_ACCOUNT_URLS = TON_ASSETS_ACCOUNT_FILES.map(
  fileName => `${TON_ASSETS_ACCOUNTS_BASE_URL}/${fileName}`,
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

export const readTonAssets = async (read: TextReader = readText): Promise<AddressSource> =>
  readSource("ton-assets", TON_ASSETS_ACCOUNT_URLS, parseTonAssets, read)

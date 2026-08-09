import {parseAllDocuments} from "yaml"

import type {AddressSource, TextReader} from "./shared.ts"
import {parseSourceAddresses, readSource, readText} from "./shared.ts"

const ADDRESS_BOOK_BASE_URL =
  "https://raw.githubusercontent.com/catchain/address-book/master/source"

const ADDRESS_BOOK_FILES = ["community", "exchanges", "people", "system", "validators"] as const

export const ADDRESS_BOOK_URLS = ADDRESS_BOOK_FILES.map(
  fileName => `${ADDRESS_BOOK_BASE_URL}/${fileName}.yaml`,
)

export const parseAddressBook = (text: string, sourcePath: string) => {
  try {
    // Match the upstream address-book generator: recover parsed documents without
    // rejecting YAML errors, including unquoted names that start with `@`.
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

export const readAddressBook = async (read: TextReader = readText): Promise<AddressSource> =>
  readSource("address-book", ADDRESS_BOOK_URLS, parseAddressBook, read)

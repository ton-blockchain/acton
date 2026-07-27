import type {AddressSource, TextReader} from "./shared.ts"
import {parseYamlAddresses, readSource, readText} from "./shared.ts"

const ADDRESS_BOOK_BASE_URL =
  "https://raw.githubusercontent.com/catchain/address-book/master/source"

const ADDRESS_BOOK_FILES = [
  "community.yaml",
  "exchanges.yaml",
  "people.yaml",
  "system.yaml",
  "validators.yaml",
] as const

export const ADDRESS_BOOK_URLS = ADDRESS_BOOK_FILES.map(
  fileName => `${ADDRESS_BOOK_BASE_URL}/${fileName}`,
)

export const parseAddressBook = parseYamlAddresses

export const readAddressBook = async (read: TextReader = readText): Promise<AddressSource> =>
  readSource("address-book", ADDRESS_BOOK_URLS, parseAddressBook, read)

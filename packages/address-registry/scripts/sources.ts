import {readActon} from "./sources/acton.ts"
import {readTonAssets} from "./sources/ton-assets.ts"
import {readAddressBook} from "./sources/address-book.ts"
import type {AddressSource, TextReader} from "./sources/shared.ts"
import {readText} from "./sources/shared.ts"

export const readSources = async (
  read: TextReader = readText,
): Promise<readonly [AddressSource, AddressSource, AddressSource]> =>
  Promise.all([readTonAssets(read), readAddressBook(read), readActon()])

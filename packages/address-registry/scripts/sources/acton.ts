import type {AddressSource, SourceAddress} from "./shared.ts"
import {parseSourceAddresses} from "./shared.ts"

export const ACTON_ADDRESSES = [
  {
    address: "kQCSES0TZYqcVkgoguhIb8iMEo4cvaEwmIrU5qbQgnN8ftBF",
    name: "Testgiver TON Bot",
  },
  {
    address: "kQD_O1WeM-icMY8JIoGzgySEQ8ivvoSpgSoglUsaua6YDBtX",
    name: "Acton Faucet",
  },
] as const satisfies readonly SourceAddress[]

export const readActon = (): AddressSource => ({
  id: "acton",
  urls: [],
  addresses: parseSourceAddresses(ACTON_ADDRESSES, "ACTON_ADDRESSES"),
})

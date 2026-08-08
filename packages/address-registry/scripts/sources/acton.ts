import type {AddressSource, SourceAddress} from "./shared.ts"
import {parseSourceAddresses} from "./shared.ts"

export const ACTON_MAINNET_ADDRESSES = [] as const satisfies readonly SourceAddress[]

export const ACTON_TESTNET_ADDRESSES = [
  {
    address: "kf_v5x0Thgr6pq6ur2NvkWhIf4DxAxsL-Nk5rknT6n99oEkd",
    name: "Root DNS",
  },
  {
    address: "kQCSES0TZYqcVkgoguhIb8iMEo4cvaEwmIrU5qbQgnN8ftBF",
    name: "Testgiver TON Bot",
  },
  {
    address: "kQD_O1WeM-icMY8JIoGzgySEQ8ivvoSpgSoglUsaua6YDBtX",
    name: "Acton Faucet",
  },
] as const satisfies readonly SourceAddress[]

const ACTON_ADDRESSES = [...ACTON_MAINNET_ADDRESSES, ...ACTON_TESTNET_ADDRESSES]

export const readActon = (): AddressSource => ({
  id: "acton",
  urls: [],
  addresses: parseSourceAddresses(ACTON_ADDRESSES, "ACTON_ADDRESSES"),
})

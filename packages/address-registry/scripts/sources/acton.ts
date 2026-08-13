import type {AddressSource, SourceAddress} from "./shared.ts"
import {parseSourceAddresses} from "./shared.ts"

export const ACTON_MAINNET_ADDRESSES = [
  // https://github.com/ton-blockchain/token-bridge/blob/63c9d57fee76d7f7e1bb9e39d6c8f56ba4affa97/src/utils/constants.ts#L16
  {
    address: "Ef-ozmw2qoulNrKlqDMJimwY-a41I9y0Q_CsOHF9rdjEEaOi",
    name: "Ethereum Bridge V2 Multisig",
  },
  {
    address: "Ef-3TdlZP5vY6qLFFCESWKDqOMcSUuv4djYlFLx3QsjfrU6p",
    name: "Ethereum Bridge Multisig",
  },
  {
    address: "Ef_FD4kDZsgfXEaQxoPYlMKUCnZ__0famrsKSjwSXUmWv3tA",
    name: "BSC Bridge Multisig",
  },
  // https://telegra.ph/July-2025-update-proposal-06-30
  {
    address: "Ef_q19o4m94xfF-yhYB85Qe6rTHDX-VTSzxBh4XpAfZMaOvk",
    name: "BTC Teleport Coordinator",
  },
  {
    address: "EQCxE6mUtQJKFnGfaROTKOt1lZbDiiX1kCixRv7Nw2Id_sDs",
    name: "Tether USD (USDT)",
  },
] as const satisfies readonly SourceAddress[]

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

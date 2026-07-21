import type {Wallet} from "@ton/walletkit"

import type {StartupWallet} from "../explorer/api/types"

export type SupportedWalletVersion = "v4r2" | "v5r1"

export interface StartupWalletRecord extends StartupWallet {
  readonly version: SupportedWalletVersion
}

export interface RuntimeWallet {
  readonly id: string
  readonly record: StartupWalletRecord
  readonly wallet: Wallet
}

export function isSupportedWalletVersion(version: string): version is SupportedWalletVersion {
  return version === "v4r2" || version === "v5r1"
}

import type {Wallet} from "@ton/walletkit"

import type {StudioWallet, StudioWalletVersion} from "../../studioApi"

export interface ProjectWalletRecord extends StudioWallet {
  readonly version: StudioWalletVersion
}

export interface RuntimeWallet {
  readonly id: string
  readonly record: ProjectWalletRecord
  readonly wallet: Wallet
}

export function isSupportedWalletVersion(version: string): version is StudioWalletVersion {
  return version === "v4r2" || version === "v5r1"
}

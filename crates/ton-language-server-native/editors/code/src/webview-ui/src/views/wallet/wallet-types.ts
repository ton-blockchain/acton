//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

export interface WalletInfo {
  readonly name: string
  readonly address: string
  readonly kind: string
  readonly is_global: boolean
  readonly balance: string | null
}

export interface WalletListInfo {
  readonly success: boolean
  readonly wallets: readonly WalletInfo[]
}

export type WalletMessage =
  | {
      readonly type: "updateWallets"
      readonly wallets: readonly WalletInfo[]
      readonly isLoading: boolean
    }
  | {
      readonly type: "airdropStatus"
      readonly walletName: string
      readonly status: "requesting" | "idle"
      readonly message?: string
    }
  | {readonly type: "showNewWalletForm"}
  | {readonly type: "showImportWalletForm"}

export type WebviewWalletCommand =
  | {readonly type: "loadWallets"}
  | {
      readonly type: "newWallet"
      readonly name: string
      readonly version: string
      readonly global: boolean
      readonly secure: boolean
    }
  | {
      readonly type: "importWallet"
      readonly name: string
      readonly mnemonic: string
      readonly version: string
      readonly global: boolean
      readonly secure: boolean
    }
  | {readonly type: "airdrop"; readonly walletName: string}
  | {readonly type: "copyAddress"; readonly address: string}
  | {readonly type: "openInExplorer"; readonly address: string}
  | {readonly type: "webviewReady"}

export interface WalletVSCodeAPI {
  readonly postMessage: (message: WebviewWalletCommand) => void
}

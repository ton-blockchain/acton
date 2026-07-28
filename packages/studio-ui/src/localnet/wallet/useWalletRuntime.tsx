import {createContext, useContext} from "react"

import type {TONConnectSession} from "@ton/walletkit"

import type {StudioWallet} from "../../studioApi"

import type {RuntimeWallet} from "./types"

export interface WalletBalanceState {
  readonly value?: string
  readonly isLoading: boolean
  readonly error?: string
}

export interface WalletRuntimeContextValue {
  readonly projectWallets: readonly StudioWallet[]
  readonly runtimeWallets: readonly RuntimeWallet[]
  readonly unsupportedWallets: readonly StudioWallet[]
  readonly sessions: readonly TONConnectSession[]
  readonly walletBalances: Readonly<Record<string, WalletBalanceState>>
  readonly copiedAddress?: string
  readonly tonConnectUrl: string
  readonly isLoadingWallets: boolean
  readonly isInitializing: boolean
  readonly isSyncingWallets: boolean
  readonly isSubmitting: boolean
  readonly isRefreshingBalances: boolean
  readonly pendingRequestCount: number
  readonly setTonConnectUrl: (value: string) => void
  readonly handleConnectUrl: (url: string) => Promise<void>
  readonly refreshWalletBalances: (wallets?: readonly RuntimeWallet[]) => Promise<void>
  readonly handleDisconnectSession: (sessionId: string) => Promise<void>
  readonly handleCopyAddress: (address: string) => Promise<void>
}

export const WalletRuntimeContext = createContext<WalletRuntimeContextValue | undefined>(undefined)

export function useWalletRuntime(): WalletRuntimeContextValue {
  const value = useContext(WalletRuntimeContext)
  if (!value) {
    throw new Error("useWalletRuntime must be used inside WalletRuntimeProvider")
  }
  return value
}

export function useOptionalWalletRuntime(): WalletRuntimeContextValue | undefined {
  return useContext(WalletRuntimeContext)
}

import {createContext, useContext} from "react"

import type {LocalnetNodeInfo} from "../api/types"
import type {AddressFormatOptions} from "../components/utils"

export type BuiltinExplorerNetworkId = "mainnet" | "testnet" | "localnet"
export type CustomExplorerNetworkId = `custom:${string}`
export type ExplorerNetworkId = BuiltinExplorerNetworkId | CustomExplorerNetworkId

export interface ExplorerApiConfig {
  readonly v2BaseUrl: string
  readonly v3BaseUrl: string
  readonly toncenterApiKey?: string
}

export interface ExplorerNetworkInfo {
  readonly id: ExplorerNetworkId
  readonly label: string
  readonly testOnly: boolean
  readonly supportsActions: boolean
  readonly api?: ExplorerApiConfig
}

export interface NetworkInfoContextValue {
  readonly nodeInfo?: LocalnetNodeInfo
  readonly forkNetwork?: string
  readonly isMainnetFork: boolean
  readonly addressFormat: AddressFormatOptions
  readonly network: ExplorerNetworkInfo
}

export const MAINNET_EXPLORER_NETWORK: ExplorerNetworkInfo = {
  id: "mainnet",
  label: "Mainnet",
  testOnly: false,
  supportsActions: true,
}

export const TESTNET_EXPLORER_NETWORK: ExplorerNetworkInfo = {
  id: "testnet",
  label: "Testnet",
  testOnly: true,
  supportsActions: true,
}

const fallbackAddressFormat: AddressFormatOptions = {
  testOnly: MAINNET_EXPLORER_NETWORK.testOnly,
}

const fallbackNetworkInfo: NetworkInfoContextValue = {
  isMainnetFork: true,
  addressFormat: fallbackAddressFormat,
  network: MAINNET_EXPLORER_NETWORK,
}

export const NetworkInfoContext = createContext<NetworkInfoContextValue>(fallbackNetworkInfo)

export function useNetworkInfo(): NetworkInfoContextValue {
  return useContext(NetworkInfoContext)
}

export function useAddressFormat(): AddressFormatOptions {
  return useNetworkInfo().addressFormat
}

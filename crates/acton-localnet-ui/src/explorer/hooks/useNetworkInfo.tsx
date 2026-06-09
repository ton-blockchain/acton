import {createContext, useContext} from "react"

import type {LocalnetNodeInfo} from "../api/types"
import type {AddressFormatOptions} from "../components/utils"

export interface NetworkInfoContextValue {
  readonly nodeInfo?: LocalnetNodeInfo
  readonly forkNetwork?: string
  readonly isMainnetFork: boolean
  readonly addressFormat: AddressFormatOptions
}

const fallbackAddressFormat: AddressFormatOptions = {
  testOnly: true,
}

const fallbackNetworkInfo: NetworkInfoContextValue = {
  isMainnetFork: false,
  addressFormat: fallbackAddressFormat,
}

export const NetworkInfoContext = createContext<NetworkInfoContextValue>(fallbackNetworkInfo)

export function useNetworkInfo(): NetworkInfoContextValue {
  return useContext(NetworkInfoContext)
}

export function useAddressFormat(): AddressFormatOptions {
  return useNetworkInfo().addressFormat
}

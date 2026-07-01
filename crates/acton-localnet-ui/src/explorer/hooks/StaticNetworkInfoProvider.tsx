import type {FC, ReactNode} from "react"
import {useMemo} from "react"

import {
  type ExplorerNetworkInfo,
  MAINNET_EXPLORER_NETWORK,
  NetworkInfoContext,
  type NetworkInfoContextValue,
  TESTNET_EXPLORER_NETWORK,
} from "./useNetworkInfo"

interface StaticNetworkInfoProviderProps {
  readonly children: ReactNode
  readonly network?: ExplorerNetworkInfo
  readonly testOnly?: boolean
}

export const StaticNetworkInfoProvider: FC<StaticNetworkInfoProviderProps> = ({
  children,
  network,
  testOnly = false,
}) => {
  const resolvedNetwork = useMemo<ExplorerNetworkInfo>(() => {
    return network ?? (testOnly ? TESTNET_EXPLORER_NETWORK : MAINNET_EXPLORER_NETWORK)
  }, [network, testOnly])
  const addressFormat = useMemo(
    () => ({testOnly: resolvedNetwork.testOnly}),
    [resolvedNetwork.testOnly],
  )
  const value = useMemo<NetworkInfoContextValue>(
    () => ({
      addressFormat,
      isMainnetFork: !resolvedNetwork.testOnly,
      network: resolvedNetwork,
    }),
    [addressFormat, resolvedNetwork],
  )

  return <NetworkInfoContext.Provider value={value}>{children}</NetworkInfoContext.Provider>
}

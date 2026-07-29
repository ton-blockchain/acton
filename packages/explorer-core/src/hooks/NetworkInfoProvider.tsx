import {useEffect, useMemo, useState} from "react"
import type {FC, ReactNode} from "react"

import type {TonClient} from "../api/client"
import type {LocalnetNodeInfo} from "../api/types"

import {
  NetworkInfoContext,
  type ExplorerApiConfig,
  type ExplorerNetworkInfo,
  type NetworkInfoContextValue,
} from "./useNetworkInfo"

interface ExplorerNetworkIdentity {
  readonly id: string
  readonly label: string
  readonly testOnly: boolean
}

interface NetworkInfoProviderProps {
  readonly client: TonClient
  readonly api: ExplorerApiConfig
  readonly children: ReactNode
  readonly enabled?: boolean
  readonly network?: ExplorerNetworkIdentity
}

export const NetworkInfoProvider: FC<NetworkInfoProviderProps> = ({
  client,
  api,
  children,
  enabled = true,
  network: networkIdentity,
}) => {
  const [nodeInfo, setNodeInfo] = useState<LocalnetNodeInfo | undefined>()

  useEffect(() => {
    if (!enabled) {
      setNodeInfo(undefined)
      return
    }

    let cancelled = false

    const loadNodeInfo = async () => {
      try {
        const nextNodeInfo = await client.getNodeInfo()
        if (!cancelled) {
          setNodeInfo(nextNodeInfo)
        }
      } catch {
        if (!cancelled) {
          setNodeInfo(undefined)
        }
      }
    }

    void loadNodeInfo()

    return () => {
      cancelled = true
    }
  }, [client, enabled])

  const forkNetwork = nodeInfo?.fork_network?.trim()
  const normalizedForkNetwork = forkNetwork?.toLocaleLowerCase()
  const isFork = nodeInfo?.state_source === "remote" && Boolean(forkNetwork)
  const baselineNetwork = useMemo<ExplorerNetworkInfo>(() => {
    const id = explorerNetworkId(networkIdentity?.id)
    return {
      id,
      label: networkIdentity?.label ?? "Localnet",
      testOnly: networkIdentity?.testOnly ?? true,
      supportsActions: id === "mainnet" || id === "testnet",
      api,
    }
  }, [api, networkIdentity])
  const isMainnetFork =
    (isFork && normalizedForkNetwork === "mainnet") || (!isFork && baselineNetwork.id === "mainnet")
  const network = useMemo<ExplorerNetworkInfo>(() => {
    if (!isFork) {
      return baselineNetwork
    }
    if (normalizedForkNetwork === "mainnet") {
      return {
        id: "mainnet",
        label: "Mainnet",
        testOnly: false,
        supportsActions: true,
        api,
      }
    }
    if (normalizedForkNetwork === "testnet") {
      return {
        id: "testnet",
        label: "Testnet",
        testOnly: true,
        supportsActions: true,
        api,
      }
    }
    return {
      id: `custom:${normalizedForkNetwork ?? "fork"}`,
      label: forkNetwork ?? "Custom",
      testOnly: true,
      supportsActions: false,
      api,
    }
  }, [api, baselineNetwork, forkNetwork, isFork, normalizedForkNetwork])
  const addressFormat = useMemo(
    () => ({
      testOnly: network.testOnly,
    }),
    [network.testOnly],
  )

  const value = useMemo<NetworkInfoContextValue>(() => {
    return {
      nodeInfo,
      forkNetwork: isFork ? forkNetwork : undefined,
      isMainnetFork,
      addressFormat,
      network,
    }
  }, [addressFormat, forkNetwork, isFork, isMainnetFork, network, nodeInfo])

  return <NetworkInfoContext.Provider value={value}>{children}</NetworkInfoContext.Provider>
}

function explorerNetworkId(id: string | undefined): ExplorerNetworkInfo["id"] {
  const normalized = id?.trim().toLocaleLowerCase()
  if (!normalized || normalized === "localnet" || normalized === "acton-localnet") {
    return "localnet"
  }
  if (normalized === "mainnet" || normalized === "testnet") return normalized
  return normalized.startsWith("custom:") ? `custom:${normalized.slice(7)}` : `custom:${normalized}`
}

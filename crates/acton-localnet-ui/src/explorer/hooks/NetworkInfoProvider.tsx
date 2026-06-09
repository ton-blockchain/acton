import type React from "react"
import {useEffect, useMemo, useState} from "react"

import type {TonClient} from "../api/client"
import type {LocalnetNodeInfo} from "../api/types"

import {NetworkInfoContext, type NetworkInfoContextValue} from "./useNetworkInfo"

interface NetworkInfoProviderProps {
  readonly client: TonClient
  readonly children: React.ReactNode
}

export const NetworkInfoProvider: React.FC<NetworkInfoProviderProps> = ({client, children}) => {
  const [nodeInfo, setNodeInfo] = useState<LocalnetNodeInfo | undefined>()

  useEffect(() => {
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
  }, [client])

  const forkNetwork = nodeInfo?.fork_network?.trim()
  const normalizedForkNetwork = forkNetwork?.toLocaleLowerCase()
  const isFork = nodeInfo?.state_source === "remote" && Boolean(forkNetwork)
  const isMainnetFork = isFork && normalizedForkNetwork === "mainnet"
  const addressFormat = useMemo(
    () => ({
      testOnly: !isMainnetFork,
    }),
    [isMainnetFork],
  )

  const value = useMemo<NetworkInfoContextValue>(() => {
    return {
      nodeInfo,
      forkNetwork: isFork ? forkNetwork : undefined,
      isMainnetFork,
      addressFormat,
    }
  }, [addressFormat, forkNetwork, isFork, isMainnetFork, nodeInfo])

  return <NetworkInfoContext.Provider value={value}>{children}</NetworkInfoContext.Provider>
}

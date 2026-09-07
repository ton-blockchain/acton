import type {TonClient} from "@acton/explorer-core/api/client"
import type {LocalnetContract} from "@acton/explorer-core/api/types"
import {useToast} from "@acton/ui"
import {useEffect, useState} from "react"

import {supports} from "../environmentCapabilities"
import {fetchStudioWallets, type StudioEnvironment, type StudioWallet} from "../studioApi"

interface SearchObjects {
  readonly client: TonClient
  readonly environmentId?: string
  readonly contracts: readonly LocalnetContract[]
  readonly wallets: readonly StudioWallet[]
  readonly loaded: boolean
  readonly failed: boolean
}

/** Refresh only the open search's current registry; never wake or query other environments */
export function useSearchObjects(
  client: TonClient,
  environment: StudioEnvironment | undefined,
  open: boolean,
) {
  const {showToast} = useToast()
  const [revision, setRevision] = useState(0)
  const [state, setState] = useState<SearchObjects>()
  const environmentId = environment?.id
  const running = environment?.status === "running"
  const contractsEnabled = supports(environment, "contracts")
  const walletsEnabled = supports(environment, "wallets")

  useEffect(() => {
    if (!open || !running || !environmentId || (!contractsEnabled && !walletsEnabled)) return

    const controller = new AbortController()
    const reportedErrors = new Set<string>()
    let timer: ReturnType<typeof setTimeout> | undefined
    setState({client, environmentId, contracts: [], wallets: [], loaded: false, failed: false})

    const refresh = async () => {
      if (document.visibilityState === "visible") {
        const [contracts, wallets] = await Promise.allSettled([
          contractsEnabled ? client.listContracts(controller.signal) : Promise.resolve([]),
          walletsEnabled
            ? fetchStudioWallets(environmentId, controller.signal)
            : Promise.resolve([]),
        ])
        if (controller.signal.aborted) return

        for (const [name, result] of [
          ["contracts", contracts],
          ["wallets", wallets],
        ] as const) {
          if (result.status === "rejected" && !reportedErrors.has(name)) {
            reportedErrors.add(name)
            showToast({
              title: `Could not load ${name} for search`,
              description:
                result.reason instanceof Error
                  ? result.reason.message
                  : "Try again to reload search results",
              variant: "error",
            })
          } else if (result.status === "fulfilled") {
            reportedErrors.delete(name)
          }
        }

        // Drop unavailable records instead of presenting pre-restore data as current state.
        setState({
          client,
          environmentId,
          contracts: contracts.status === "fulfilled" ? contracts.value : [],
          wallets: wallets.status === "fulfilled" ? wallets.value : [],
          loaded: true,
          failed: contracts.status === "rejected" || wallets.status === "rejected",
        })
      }

      if (!controller.signal.aborted) timer = setTimeout(() => void refresh(), 5000)
    }

    void refresh()

    return () => {
      controller.abort()
      clearTimeout(timer)
    }
  }, [client, contractsEnabled, environmentId, open, revision, running, showToast, walletsEnabled])

  // A route or token change must hide the previous client's results before its effect runs.
  const current =
    running && state?.client === client && state.environmentId === environmentId ? state : undefined
  return {
    contracts: current?.contracts ?? [],
    wallets: current?.wallets ?? [],
    loading: open && running && (contractsEnabled || walletsEnabled) && !current?.loaded,
    loaded: Boolean(current?.loaded),
    failed: current?.failed ?? false,
    retry: () => setRevision(value => value + 1),
  }
}

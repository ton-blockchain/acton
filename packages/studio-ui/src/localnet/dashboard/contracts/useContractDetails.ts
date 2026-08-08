import {useCallback, useEffect, useRef, useState} from "react"

import type {TonClient} from "@acton/explorer-core/api/client"
import type {ExtendedContractABI} from "@acton/explorer-core/api/compilerAbi"
import type {LocalnetContract} from "@acton/explorer-core/api/types"
import {isSameAddress} from "@acton/explorer-core/components/utils"
import {normalizeCodeHash} from "@acton/explorer-core/metadata/codeHash"
import {useMetadataRegistry} from "@acton/explorer-core/metadata/MetadataRegistryProvider"
import type {RegisteredSource} from "@acton/explorer-core/metadata/types"

export interface ContractDetails {
  readonly abi: ExtendedContractABI | null
  readonly abiError?: string
  readonly contract: LocalnetContract
  readonly currentSource?: RegisteredSource
  readonly deployedSource?: RegisteredSource
  readonly sourceError?: string
}

interface ContractDetailsState {
  readonly details?: ContractDetails
  readonly error?: string
  readonly loading: boolean
}

export function useContractDetails(client: TonClient, address: string) {
  const metadataRegistry = useMetadataRegistry()
  const latestLoadId = useRef(0)
  const [state, setState] = useState<ContractDetailsState>({loading: true})
  const fetchContract = useCallback(async () => {
    const contracts = await client.listContracts()
    const contract = contracts.find(candidate => isSameAddress(candidate.address, address))
    if (!contract) {
      throw new Error("This contract is not available in the current environment")
    }
    return contract
  }, [address, client])

  const reload = useCallback(
    async (showLoading = true) => {
      const loadId = ++latestLoadId.current
      if (showLoading) {
        setState({loading: true})
      }

      try {
        const contract = await fetchContract()
        if (loadId !== latestLoadId.current) return
        setState(current => ({
          loading: false,
          details:
            current.details && !showLoading
              ? {...current.details, contract}
              : {abi: null, contract},
        }))

        const codeHash = contract.codeHash.trim()
        const [sourcesResult, abiResult] = await Promise.allSettled([
          metadataRegistry.listSources(),
          codeHash
            ? metadataRegistry.getCompilerAbis([codeHash])
            : Promise.resolve<Record<string, ExtendedContractABI | null>>({}),
        ])
        const sources = sourcesResult.status === "fulfilled" ? sourcesResult.value : []
        const deployedSource = findDeployedSource(contract, sources)
        const currentSource = findCurrentSource(contract, deployedSource, sources)
        const abi =
          codeHash && abiResult.status === "fulfilled" ? (abiResult.value[codeHash] ?? null) : null

        if (loadId !== latestLoadId.current) return
        setState({
          loading: false,
          details: {
            abi,
            abiError: rejectedMessage(abiResult),
            contract,
            currentSource,
            deployedSource,
            sourceError: rejectedMessage(sourcesResult),
          },
        })
      } catch (error) {
        if (loadId !== latestLoadId.current) return
        setState(current =>
          current.details && !showLoading
            ? {...current, loading: false}
            : {
                error: error instanceof Error ? error.message : "Failed to load contract",
                loading: false,
              },
        )
      }
    },
    [fetchContract, metadataRegistry],
  )

  const refreshContract = useCallback(async () => {
    try {
      const contract = await fetchContract()
      setState(current => {
        if (!current.details || !isSameAddress(current.details.contract.address, address)) {
          return current
        }
        return {...current, details: {...current.details, contract}}
      })
    } catch {
      // Keep the last successful snapshot during background refreshes.
    }
  }, [address, fetchContract])

  useEffect(() => {
    void reload()
    return () => {
      latestLoadId.current += 1
    }
  }, [reload])

  useEffect(() => {
    const refreshWhenVisible = () => {
      if (document.visibilityState === "visible") void refreshContract()
    }
    const interval = globalThis.setInterval(refreshWhenVisible, 3000)
    globalThis.addEventListener("focus", refreshWhenVisible)
    document.addEventListener("visibilitychange", refreshWhenVisible)

    return () => {
      globalThis.clearInterval(interval)
      globalThis.removeEventListener("focus", refreshWhenVisible)
      document.removeEventListener("visibilitychange", refreshWhenVisible)
    }
  }, [refreshContract])

  return {...state, reload}
}

function findDeployedSource(
  contract: LocalnetContract,
  sources: readonly RegisteredSource[],
): RegisteredSource | undefined {
  const artifactId = contract.artifact?.artifactId
  if (artifactId) {
    const exactArtifact = sources.find(source => source.artifactId === artifactId)
    if (exactArtifact) return exactArtifact
  }

  const deployedCodeHash = normalizeCodeHash(contract.codeHash)
  if (!deployedCodeHash) return undefined
  return sources.find(source => normalizeCodeHash(source.codeHash) === deployedCodeHash)
}

function findCurrentSource(
  contract: LocalnetContract,
  deployedSource: RegisteredSource | undefined,
  sources: readonly RegisteredSource[],
): RegisteredSource | undefined {
  const entrypoint =
    deployedSource?.source.bundle?.entrypoint.trim() || contract.artifact?.entrypoint?.trim()
  if (!entrypoint) return deployedSource

  return sources
    .filter(source => source.source.bundle?.entrypoint.trim() === entrypoint)
    .toSorted((left, right) => right.savedAt - left.savedAt)[0]
}

function rejectedMessage(result: PromiseSettledResult<unknown>): string | undefined {
  if (result.status === "fulfilled") return undefined
  return result.reason instanceof Error ? result.reason.message : "Request failed"
}

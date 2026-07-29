import {createContext, useCallback, useContext, useEffect, useMemo, useState} from "react"
import type {FC, ReactNode} from "react"

import {supports} from "../environmentCapabilities"
import type {StudioEnvironment} from "../studioApi"
import {TonClient} from "@acton/explorer-core/api/client"
import {getBundledCompilerAbis} from "@acton/explorer-core/api/compilerAbiCatalog"
import {ExplorerRoutesProvider} from "@acton/explorer-core/hooks/useExplorerRoutes"
import {NetworkInfoProvider} from "@acton/explorer-core/hooks/NetworkInfoProvider"
import {BundledAbiRegistry} from "@acton/explorer-core/metadata/bundledAbiRegistry"
import {CompositeMetadataRegistry} from "@acton/explorer-core/metadata/compositeRegistry"
import {EnvironmentMetadataRegistry} from "@acton/explorer-core/metadata/environmentRegistry"
import type {ExplorerMetadataRegistry} from "@acton/explorer-core/metadata/types"
import {VerifierMetadataRegistry} from "@acton/explorer-core/metadata/verifierRegistry"
import {LocalnetRoutesProvider, localnetPath} from "./routes"

const DEFAULT_RPC_BASE_URL = globalThis.location.origin

const apiTokenStorageKey = (environmentId: string) =>
  `actonStudioEnvironment:${environmentId}:apiToken`

export interface LocalnetRuntime {
  readonly apiV2BaseUrl: string
  readonly apiV3BaseUrl: string
  readonly clearAuthToken: () => void
  readonly client: TonClient
  readonly closeAuthOverlay: () => void
  readonly environment?: StudioEnvironment
  readonly gramFaucetEnabled: boolean
  readonly isAuthOverlayOpen: boolean
  readonly isAuthOverlayRequired: boolean
  readonly jettonFaucetEnabled: boolean
  readonly localnetApiToken?: string
  readonly metadataRegistry: CompositeMetadataRegistry
  readonly openAuthOverlay: () => void
  readonly rpcBaseUrl: string
  readonly requireAuthToken: () => void
  readonly saveAuthToken: (token: string) => void
}

const LocalnetRuntimeContext = createContext<LocalnetRuntime | undefined>(undefined)

interface LocalnetRuntimeProviderProps {
  readonly basePath?: string
  readonly children: ReactNode
  readonly environment?: StudioEnvironment
}

export const LocalnetRuntimeProvider: FC<LocalnetRuntimeProviderProps> = ({
  basePath = "",
  children,
  environment,
}) => {
  const environmentId = environment?.id
  const rpcBaseUrl = useMemo(
    () =>
      environment
        ? new URL(environment.rpcUrl, globalThis.location.origin).toString().replace(/\/$/, "")
        : DEFAULT_RPC_BASE_URL,
    [environment],
  )
  const apiV2BaseUrl = environment?.endpoints.apiV2 ?? `${rpcBaseUrl}/api/v2`
  const apiV3BaseUrl = environment?.endpoints.apiV3 ?? `${rpcBaseUrl}/api/v3`
  const controlBaseUrl = environment?.endpoints.control ?? rpcBaseUrl
  const controlEnabled = supports(environment, "controlApi")
  const contractsEnabled = supports(environment, "contracts")
  const gramFaucetEnabled = supports(environment, "gramFaucet")
  const jettonFaucetEnabled = supports(environment, "jettonFaucet")
  const networkIdentity = useMemo(
    () => environment?.network,
    [environment?.network.id, environment?.network.label, environment?.network.testOnly],
  )
  const [localnetApiToken, setLocalnetApiTokenState] = useState<string>()
  const [isAuthOverlayOpen, setIsAuthOverlayOpen] = useState(false)
  const [isAuthOverlayRequired, setIsAuthOverlayRequired] = useState(false)

  useEffect(() => {
    setLocalnetApiTokenState(
      environmentId
        ? localStorage.getItem(apiTokenStorageKey(environmentId)) || undefined
        : undefined,
    )
    setIsAuthOverlayOpen(false)
    setIsAuthOverlayRequired(false)
  }, [environmentId])

  const setLocalnetApiToken = useCallback(
    (token: string | undefined) => {
      const nextToken = token?.trim() || undefined
      if (environmentId) {
        const storageKey = apiTokenStorageKey(environmentId)
        if (nextToken) {
          localStorage.setItem(storageKey, nextToken)
        } else {
          localStorage.removeItem(storageKey)
        }
      }
      setLocalnetApiTokenState(nextToken)
    },
    [environmentId],
  )

  const openAuthOverlay = useCallback(() => {
    setIsAuthOverlayRequired(false)
    setIsAuthOverlayOpen(true)
  }, [])

  const closeAuthOverlay = useCallback(() => {
    setIsAuthOverlayRequired(false)
    setIsAuthOverlayOpen(false)
  }, [])

  const requireAuthToken = useCallback(() => {
    setIsAuthOverlayRequired(true)
    setIsAuthOverlayOpen(true)
  }, [])

  const saveAuthToken = useCallback(
    (token: string) => {
      setLocalnetApiToken(token)
      setIsAuthOverlayRequired(false)
      setIsAuthOverlayOpen(false)
    },
    [setLocalnetApiToken],
  )

  const clearAuthToken = useCallback(() => {
    setLocalnetApiToken(undefined)
    if (!isAuthOverlayRequired) setIsAuthOverlayOpen(false)
  }, [isAuthOverlayRequired, setLocalnetApiToken])

  const client = useMemo(
    () =>
      new TonClient({
        v2BaseUrl: apiV2BaseUrl,
        v3BaseUrl: apiV3BaseUrl,
        addressNameBaseUrl: controlBaseUrl,
        localnetControlEnabled: controlEnabled,
        toncenterApiCompatible: !controlEnabled,
        localnetApiToken,
        onUnauthorized: requireAuthToken,
      }),
    [
      apiV2BaseUrl,
      apiV3BaseUrl,
      controlBaseUrl,
      controlEnabled,
      localnetApiToken,
      requireAuthToken,
    ],
  )
  const metadataRegistry = useMemo(() => {
    const registries: ExplorerMetadataRegistry[] = [
      new BundledAbiRegistry(getBundledCompilerAbis),
      new VerifierMetadataRegistry(),
    ]
    if (contractsEnabled) {
      registries.unshift(new EnvironmentMetadataRegistry(client))
    }
    return new CompositeMetadataRegistry(registries)
  }, [client, contractsEnabled])
  const explorerApi = useMemo(
    () => ({
      v2BaseUrl: apiV2BaseUrl,
      v3BaseUrl: apiV3BaseUrl,
    }),
    [apiV2BaseUrl, apiV3BaseUrl],
  )
  const value = useMemo<LocalnetRuntime>(
    () => ({
      apiV2BaseUrl,
      apiV3BaseUrl,
      clearAuthToken,
      client,
      closeAuthOverlay,
      environment,
      gramFaucetEnabled,
      isAuthOverlayOpen,
      isAuthOverlayRequired,
      jettonFaucetEnabled,
      localnetApiToken,
      metadataRegistry,
      openAuthOverlay,
      rpcBaseUrl,
      requireAuthToken,
      saveAuthToken,
    }),
    [
      apiV2BaseUrl,
      apiV3BaseUrl,
      clearAuthToken,
      client,
      closeAuthOverlay,
      environment,
      gramFaucetEnabled,
      isAuthOverlayOpen,
      isAuthOverlayRequired,
      jettonFaucetEnabled,
      localnetApiToken,
      metadataRegistry,
      openAuthOverlay,
      rpcBaseUrl,
      requireAuthToken,
      saveAuthToken,
    ],
  )

  return (
    <LocalnetRuntimeContext.Provider value={value}>
      <NetworkInfoProvider
        client={client}
        api={explorerApi}
        enabled={environment?.status === "running" && controlEnabled}
        network={networkIdentity}
      >
        <ExplorerRoutesProvider
          abiPath={localnetPath(basePath, "/contracts/abi")}
          basePath={localnetPath(basePath, "/explorer")}
          cellPath={localnetPath(basePath, "/cell-inspector")}
          contractsPath={localnetPath(basePath, "/contracts")}
          emulatePath={localnetPath(basePath, "/simulator")}
          localnetBasePath={basePath}
          sourcesPath={localnetPath(basePath, "/contracts/sources")}
        >
          <LocalnetRoutesProvider basePath={basePath}>{children}</LocalnetRoutesProvider>
        </ExplorerRoutesProvider>
      </NetworkInfoProvider>
    </LocalnetRuntimeContext.Provider>
  )
}

export function useLocalnetRuntime() {
  const runtime = useContext(LocalnetRuntimeContext)
  if (!runtime) throw new Error("Localnet runtime is not available")
  return runtime
}

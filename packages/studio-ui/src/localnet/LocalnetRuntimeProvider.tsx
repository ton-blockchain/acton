import {createContext, useCallback, useContext, useEffect, useMemo, useState} from "react"
import type {FC, ReactNode} from "react"

import type {StudioEnvironment} from "../studioApi"
import {TonClient} from "./explorer/api/client"
import {getBundledCompilerAbis} from "./explorer/api/compilerAbiCatalog"
import {ExplorerRoutesProvider} from "./explorer/hooks/useExplorerRoutes"
import {NetworkInfoProvider} from "./explorer/hooks/NetworkInfoProvider"
import {BundledAbiRegistry} from "./explorer/metadata/bundledAbiRegistry"
import {CompositeMetadataRegistry} from "./explorer/metadata/compositeRegistry"
import {LocalnetMetadataRegistry} from "./explorer/metadata/localnetRegistry"
import {VerifierMetadataRegistry} from "./explorer/metadata/verifierRegistry"
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
  readonly isAuthOverlayOpen: boolean
  readonly isAuthOverlayRequired: boolean
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
  const apiV2BaseUrl = `${rpcBaseUrl}/api/v2`
  const apiV3BaseUrl = `${rpcBaseUrl}/api/v3`
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
        addressNameBaseUrl: rpcBaseUrl,
        localnetApiToken,
        onUnauthorized: requireAuthToken,
      }),
    [apiV2BaseUrl, apiV3BaseUrl, localnetApiToken, requireAuthToken, rpcBaseUrl],
  )
  const metadataRegistry = useMemo(
    () =>
      new CompositeMetadataRegistry([
        new LocalnetMetadataRegistry(client),
        new BundledAbiRegistry(getBundledCompilerAbis),
        new VerifierMetadataRegistry(),
      ]),
    [client],
  )
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
      isAuthOverlayOpen,
      isAuthOverlayRequired,
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
      isAuthOverlayOpen,
      isAuthOverlayRequired,
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
        enabled={environment?.status === "running"}
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

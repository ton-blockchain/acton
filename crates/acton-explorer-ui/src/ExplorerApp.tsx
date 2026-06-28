import {ThemeSwitch, ToastProvider, useToast} from "@acton/shared-ui"
import {Check, ChevronDown, Edit2, Github, Plus, Share2, Trash2} from "lucide-react"
import {useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState} from "react"
import type {FC, ReactNode} from "react"
import {BrowserRouter, Link, Navigate, Route, Routes, useLocation} from "react-router-dom"

import {TonClient} from "../../acton-localnet-ui/src/explorer/api/client"
import {getBundledCompilerAbis} from "../../acton-localnet-ui/src/explorer/api/compilerAbiCatalog"
import {AddressBookProvider} from "../../acton-localnet-ui/src/explorer/hooks/useAddressBook"
import {ExplorerRoutesProvider} from "../../acton-localnet-ui/src/explorer/hooks/useExplorerRoutes"
import {StaticNetworkInfoProvider} from "../../acton-localnet-ui/src/explorer/hooks/StaticNetworkInfoProvider"
import {BrowserMetadataRegistry} from "../../acton-localnet-ui/src/explorer/metadata/browserRegistry"
import {BundledAbiRegistry} from "../../acton-localnet-ui/src/explorer/metadata/bundledAbiRegistry"
import {CompositeMetadataRegistry} from "../../acton-localnet-ui/src/explorer/metadata/compositeRegistry"
import {MetadataRegistryProvider} from "../../acton-localnet-ui/src/explorer/metadata/MetadataRegistryProvider"
import {VerifierMetadataRegistry} from "../../acton-localnet-ui/src/explorer/metadata/verifierRegistry"
import type {
  CustomExplorerNetworkId,
  ExplorerApiConfig,
  ExplorerNetworkInfo,
} from "../../acton-localnet-ui/src/explorer/hooks/useNetworkInfo"
import {BlockDetailsPage, BlocksPage} from "../../acton-localnet-ui/src/explorer/pages/BlocksPage"
import {AccountPage} from "../../acton-localnet-ui/src/explorer/pages/AccountPage"
import {
  AbiCatalogPage,
  AbiDetailsPage,
} from "../../acton-localnet-ui/src/explorer/pages/AbiCatalogPage"
import {SourceCatalogPage} from "../../acton-localnet-ui/src/explorer/pages/SourceCatalogPage"
import {ExplorerSearch} from "../../acton-localnet-ui/src/explorer/components/ExplorerSearch"
import {ExplorerIndexPage} from "../../acton-localnet-ui/src/explorer/pages/ExplorerIndexPage"
import {TransactionPage} from "../../acton-localnet-ui/src/explorer/pages/TransactionPage"
import type {ThemeMode} from "@acton/shared-ui"
import "@acton/shared-ui/styles/tokens.css"
import "../../acton-localnet-ui/src/index.css"
import actonScanLogo from "./assets/acton-scan-logo-dark.svg"
import styles from "./ExplorerApp.module.css"

type BuiltinSelectableExplorerNetworkId = "mainnet" | "testnet"
type SelectableExplorerNetworkId = BuiltinSelectableExplorerNetworkId | CustomExplorerNetworkId
type SelectableExplorerNetwork = ExplorerNetworkInfo & {
  readonly id: SelectableExplorerNetworkId
  readonly api: ExplorerApiConfig
}

type NetworkFormMode =
  | {readonly type: "add"}
  | {readonly type: "edit"; readonly networkId: CustomExplorerNetworkId}

const EXPLORER_NETWORK_STORAGE_KEY = "explorerNetwork"
const EXPLORER_CUSTOM_NETWORKS_STORAGE_KEY = "explorerCustomNetworks"
const DEFAULT_CUSTOM_NETWORK_NAME = "Devnet"
const SHARED_NETWORK_NAME_QUERY_PARAM = "network.name"
const SHARED_NETWORK_V2_QUERY_PARAM = "network.v2"
const SHARED_NETWORK_V3_QUERY_PARAM = "network.v3"
const SHARED_NETWORK_TEST_ONLY_QUERY_PARAM = "network.testOnly"
const SHARED_NETWORK_ACTIONS_QUERY_PARAM = "network.actions"
const SHARED_NETWORK_QUERY_PARAMS = [
  SHARED_NETWORK_NAME_QUERY_PARAM,
  SHARED_NETWORK_V2_QUERY_PARAM,
  SHARED_NETWORK_V3_QUERY_PARAM,
  SHARED_NETWORK_TEST_ONLY_QUERY_PARAM,
  SHARED_NETWORK_ACTIONS_QUERY_PARAM,
] as const

interface StoredCustomExplorerNetwork {
  readonly id: CustomExplorerNetworkId
  readonly label: string
  readonly testOnly: boolean
  readonly supportsActions: boolean
  readonly api: ExplorerApiConfig
}

interface ExplorerNetworkState {
  readonly customNetworks: readonly SelectableExplorerNetwork[]
  readonly selectedNetworkId: SelectableExplorerNetworkId
}

const cleanApiUrl = (value: string | undefined, fallback: string): string =>
  value?.trim().replace(/\/$/, "") || fallback

const cleanApiKey = (value: string | undefined): string | undefined => value?.trim() || undefined

const normalizeCustomApiUrl = (value: string, fieldName: string): string => {
  const trimmed = value.trim()
  if (!trimmed) {
    throw new Error(`${fieldName} is required.`)
  }

  let url: URL
  try {
    url = new URL(trimmed)
  } catch {
    throw new Error(`${fieldName} must be a valid HTTP(S) URL.`)
  }

  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error(`${fieldName} must use HTTP or HTTPS.`)
  }

  return url.toString().replace(/\/+$/, "")
}

const customNetworkId = (v3BaseUrl: string): CustomExplorerNetworkId =>
  `custom:${encodeURIComponent(v3BaseUrl)}` as CustomExplorerNetworkId

const customNetworkLabel = (v3BaseUrl: string): string => {
  try {
    return new URL(v3BaseUrl).hostname
  } catch {
    return "Custom network"
  }
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const EXPLORER_API_CONFIGS = {
  mainnet: {
    id: "mainnet",
    label: "Mainnet",
    testOnly: false,
    supportsActions: true,
    api: {
      v2BaseUrl: cleanApiUrl(
        import.meta.env.VITE_EXPLORER_MAINNET_TONCENTER_API_V2_URL ??
          import.meta.env.VITE_EXPLORER_TONCENTER_API_V2_URL,
        "https://toncenter.com/api/v2",
      ),
      v3BaseUrl: cleanApiUrl(
        import.meta.env.VITE_EXPLORER_MAINNET_TONCENTER_API_V3_URL ??
          import.meta.env.VITE_EXPLORER_TONCENTER_API_V3_URL,
        "https://toncenter.com/api/v3",
      ),
      toncenterApiKey: cleanApiKey(
        import.meta.env.VITE_EXPLORER_MAINNET_TONCENTER_API_KEY ??
          import.meta.env.VITE_EXPLORER_TONCENTER_API_KEY,
      ),
    },
  },
  testnet: {
    id: "testnet",
    label: "Testnet",
    testOnly: true,
    supportsActions: true,
    api: {
      v2BaseUrl: cleanApiUrl(
        import.meta.env.VITE_EXPLORER_TESTNET_TONCENTER_API_V2_URL,
        "https://testnet.toncenter.com/api/v2",
      ),
      v3BaseUrl: cleanApiUrl(
        import.meta.env.VITE_EXPLORER_TESTNET_TONCENTER_API_V3_URL,
        "https://testnet.toncenter.com/api/v3",
      ),
      toncenterApiKey: cleanApiKey(
        import.meta.env.VITE_EXPLORER_TESTNET_TONCENTER_API_KEY ??
          import.meta.env.VITE_EXPLORER_TONCENTER_API_KEY,
      ),
    },
  },
} satisfies Record<BuiltinSelectableExplorerNetworkId, SelectableExplorerNetwork>

const EXPLORER_NETWORKS: readonly SelectableExplorerNetwork[] = [
  EXPLORER_API_CONFIGS.mainnet,
  EXPLORER_API_CONFIGS.testnet,
]

const parseStoredCustomNetwork = (value: unknown): SelectableExplorerNetwork | undefined => {
  if (!isRecord(value) || !isRecord(value.api)) {
    return undefined
  }

  const v2BaseUrl =
    typeof value.api.v2BaseUrl === "string"
      ? normalizeStoredCustomApiUrl(value.api.v2BaseUrl)
      : undefined
  const v3BaseUrl =
    typeof value.api.v3BaseUrl === "string"
      ? normalizeStoredCustomApiUrl(value.api.v3BaseUrl)
      : undefined
  if (!v2BaseUrl || !v3BaseUrl) {
    return undefined
  }

  const id =
    typeof value.id === "string" && value.id.startsWith("custom:")
      ? (value.id as CustomExplorerNetworkId)
      : customNetworkId(v3BaseUrl)
  const label =
    typeof value.label === "string" && value.label.trim()
      ? value.label.trim()
      : customNetworkLabel(v3BaseUrl)

  return {
    id,
    label,
    testOnly: value.testOnly === true,
    supportsActions: value.supportsActions === true,
    api: {
      v2BaseUrl,
      v3BaseUrl,
      toncenterApiKey:
        typeof value.api.toncenterApiKey === "string"
          ? cleanApiKey(value.api.toncenterApiKey)
          : undefined,
    },
  }
}

const normalizeStoredCustomApiUrl = (value: string): string | undefined => {
  try {
    return normalizeCustomApiUrl(value, "API URL")
  } catch {
    return undefined
  }
}

const readCustomExplorerNetworks = (): readonly SelectableExplorerNetwork[] => {
  try {
    const raw = localStorage.getItem(EXPLORER_CUSTOM_NETWORKS_STORAGE_KEY)
    if (!raw) {
      return []
    }

    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) {
      return []
    }

    return parsed.flatMap(value => {
      const network = parseStoredCustomNetwork(value)
      return network ? [network] : []
    })
  } catch {
    return []
  }
}

const serializeCustomExplorerNetwork = (
  network: SelectableExplorerNetwork,
): StoredCustomExplorerNetwork => ({
  id: network.id as CustomExplorerNetworkId,
  label: network.label,
  testOnly: network.testOnly,
  supportsActions: network.supportsActions,
  api: network.api,
})

const readSelectedExplorerNetwork = (
  networks: readonly SelectableExplorerNetwork[],
): SelectableExplorerNetworkId => {
  const storedNetwork = localStorage.getItem(EXPLORER_NETWORK_STORAGE_KEY)
  return networks.find(network => network.id === storedNetwork)?.id ?? "mainnet"
}

const readInitialExplorerNetworkState = (): ExplorerNetworkState => {
  const storedCustomNetworks = readCustomExplorerNetworks()
  const sharedNetwork = readSharedCustomExplorerNetwork()
  const customNetworks = sharedNetwork
    ? upsertCustomNetwork(storedCustomNetworks, sharedNetwork)
    : storedCustomNetworks
  return {
    customNetworks,
    selectedNetworkId:
      sharedNetwork?.id ?? readSelectedExplorerNetwork([...EXPLORER_NETWORKS, ...customNetworks]),
  }
}

const createCustomExplorerNetwork = ({
  label,
  v2BaseUrl,
  v3BaseUrl,
  toncenterApiKey,
  testOnly,
  supportsActions,
}: {
  readonly label: string
  readonly v2BaseUrl: string
  readonly v3BaseUrl: string
  readonly toncenterApiKey?: string
  readonly testOnly: boolean
  readonly supportsActions: boolean
}): SelectableExplorerNetwork => {
  const normalizedV2BaseUrl = normalizeCustomApiUrl(v2BaseUrl, "V2 endpoint")
  const normalizedV3BaseUrl = normalizeCustomApiUrl(v3BaseUrl, "V3 endpoint")
  const normalizedLabel = label.trim() || DEFAULT_CUSTOM_NETWORK_NAME
  return {
    id: customNetworkId(normalizedV3BaseUrl),
    label: normalizedLabel,
    testOnly,
    supportsActions,
    api: {
      v2BaseUrl: normalizedV2BaseUrl,
      v3BaseUrl: normalizedV3BaseUrl,
      toncenterApiKey: cleanApiKey(toncenterApiKey),
    },
  }
}

const readSharedCustomExplorerNetwork = (): SelectableExplorerNetwork | undefined => {
  const params = new URLSearchParams(globalThis.location.search)
  if (!hasSharedNetworkSearchParams(params)) {
    return undefined
  }

  const v2BaseUrl = params.get(SHARED_NETWORK_V2_QUERY_PARAM)
  const v3BaseUrl = params.get(SHARED_NETWORK_V3_QUERY_PARAM)
  if (!v2BaseUrl || !v3BaseUrl) {
    return undefined
  }

  try {
    return createCustomExplorerNetwork({
      label: params.get(SHARED_NETWORK_NAME_QUERY_PARAM) ?? DEFAULT_CUSTOM_NETWORK_NAME,
      v2BaseUrl,
      v3BaseUrl,
      toncenterApiKey: undefined,
      testOnly: isTruthyQueryValue(params.get(SHARED_NETWORK_TEST_ONLY_QUERY_PARAM)),
      supportsActions: isTruthyQueryValue(params.get(SHARED_NETWORK_ACTIONS_QUERY_PARAM)),
    })
  } catch {
    return undefined
  }
}

const hasSharedNetworkSearchParams = (params: URLSearchParams): boolean =>
  SHARED_NETWORK_QUERY_PARAMS.some(param => params.has(param))

const isTruthyQueryValue = (value: string | null): boolean => {
  const normalized = value?.trim().toLowerCase()
  return normalized === "1" || normalized === "true" || normalized === "yes" || normalized === "on"
}

const removeSharedNetworkSearchParamsFromUrl = (): void => {
  const url = new URL(globalThis.location.href)
  if (!hasSharedNetworkSearchParams(url.searchParams)) {
    return
  }

  for (const param of SHARED_NETWORK_QUERY_PARAMS) {
    url.searchParams.delete(param)
  }

  globalThis.history.replaceState(
    globalThis.history.state,
    "",
    `${url.pathname}${url.search}${url.hash}`,
  )
}

const createNetworkShareUrl = (network: SelectableExplorerNetwork): string => {
  const url = new URL(globalThis.location.href)
  for (const param of SHARED_NETWORK_QUERY_PARAMS) {
    url.searchParams.delete(param)
  }

  if (isCustomNetworkId(network.id)) {
    url.searchParams.set(SHARED_NETWORK_NAME_QUERY_PARAM, network.label)
    url.searchParams.set(SHARED_NETWORK_V2_QUERY_PARAM, network.api.v2BaseUrl)
    url.searchParams.set(SHARED_NETWORK_V3_QUERY_PARAM, network.api.v3BaseUrl)
    url.searchParams.set(SHARED_NETWORK_TEST_ONLY_QUERY_PARAM, network.testOnly ? "1" : "0")
    if (network.supportsActions) {
      url.searchParams.set(SHARED_NETWORK_ACTIONS_QUERY_PARAM, "1")
    }
  }

  return url.toString()
}

function upsertCustomNetwork(
  networks: readonly SelectableExplorerNetwork[],
  network: SelectableExplorerNetwork,
): readonly SelectableExplorerNetwork[] {
  return [...networks.filter(customNetwork => customNetwork.id !== network.id), network]
}

function apiHostname(network: SelectableExplorerNetwork): string {
  try {
    return new URL(network.api.v3BaseUrl).hostname
  } catch {
    return network.api.v3BaseUrl
  }
}

const readInitialTheme = (): ThemeMode => {
  const storedTheme = localStorage.getItem("explorerTheme")
  if (storedTheme === "dark" || storedTheme === "light") {
    return storedTheme
  }

  return globalThis.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
}

function NetworkDropdown({
  networks,
  value,
  onChange,
  onAddNetwork,
  onEditNetwork,
  onDeleteNetwork,
}: {
  readonly networks: readonly SelectableExplorerNetwork[]
  readonly value: SelectableExplorerNetworkId
  readonly onChange: (network: SelectableExplorerNetworkId) => void
  readonly onAddNetwork: (network: SelectableExplorerNetwork) => void
  readonly onEditNetwork: (
    previousNetworkId: CustomExplorerNetworkId,
    network: SelectableExplorerNetwork,
  ) => void
  readonly onDeleteNetwork: (network: CustomExplorerNetworkId) => void
}) {
  const {showToast} = useToast()
  const [open, setOpen] = useState(false)
  const [networkFormMode, setNetworkFormMode] = useState<NetworkFormMode | undefined>()
  const [customName, setCustomName] = useState(DEFAULT_CUSTOM_NETWORK_NAME)
  const [customV2Url, setCustomV2Url] = useState("")
  const [customV3Url, setCustomV3Url] = useState("")
  const [customApiKey, setCustomApiKey] = useState("")
  const [customTestOnly, setCustomTestOnly] = useState(true)
  const [customSupportsActions, setCustomSupportsActions] = useState(false)
  const dropdownRef = useRef<HTMLDivElement>(null)
  const selectedNetwork = networks.find(network => network.id === value) ?? networks[0]
  const isNetworkFormOpen = networkFormMode !== undefined
  const resetNetworkForm = useCallback(() => {
    setNetworkFormMode(undefined)
    setCustomName(DEFAULT_CUSTOM_NETWORK_NAME)
    setCustomV2Url("")
    setCustomV3Url("")
    setCustomApiKey("")
    setCustomTestOnly(true)
    setCustomSupportsActions(false)
  }, [])
  const closeDropdown = useCallback(() => {
    setOpen(false)
    resetNetworkForm()
  }, [resetNetworkForm])

  const startAddNetwork = useCallback(() => {
    setNetworkFormMode({type: "add"})
    setCustomName(DEFAULT_CUSTOM_NETWORK_NAME)
    setCustomV2Url("")
    setCustomV3Url("")
    setCustomApiKey("")
    setCustomTestOnly(true)
    setCustomSupportsActions(false)
  }, [])

  const startEditNetwork = useCallback((network: SelectableExplorerNetwork) => {
    if (!isCustomNetworkId(network.id)) {
      return
    }

    setNetworkFormMode({type: "edit", networkId: network.id})
    setCustomName(network.label)
    setCustomV2Url(network.api.v2BaseUrl)
    setCustomV3Url(network.api.v3BaseUrl)
    setCustomApiKey(network.api.toncenterApiKey ?? "")
    setCustomTestOnly(network.testOnly)
    setCustomSupportsActions(network.supportsActions)
  }, [])

  const handleSaveCustomNetwork = useCallback(() => {
    if (!networkFormMode) {
      return
    }

    try {
      const network = createCustomExplorerNetwork({
        label: customName,
        v2BaseUrl: customV2Url,
        v3BaseUrl: customV3Url,
        toncenterApiKey: customApiKey,
        testOnly: customTestOnly,
        supportsActions: customSupportsActions,
      })
      if (networkFormMode.type === "edit") {
        onEditNetwork(networkFormMode.networkId, network)
      } else {
        onAddNetwork(network)
      }
      closeDropdown()
      showToast({
        description: networkFormMode.type === "edit" ? "Network updated." : "Network added.",
        variant: "success",
      })
    } catch (error) {
      showToast({
        title: "Invalid network",
        description: error instanceof Error ? error.message : "Check Toncenter API URLs.",
        variant: "error",
      })
    }
  }, [
    closeDropdown,
    customApiKey,
    customName,
    customSupportsActions,
    customTestOnly,
    customV2Url,
    customV3Url,
    networkFormMode,
    onAddNetwork,
    onEditNetwork,
    showToast,
  ])

  const handleCopyShareLink = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(createNetworkShareUrl(selectedNetwork))
      closeDropdown()
      showToast({
        description:
          isCustomNetworkId(selectedNetwork.id) && selectedNetwork.api.toncenterApiKey
            ? "Link with network copied without API key."
            : "Link with network copied.",
        variant: "success",
      })
    } catch {
      showToast({
        title: "Copy failed",
        description: "Could not copy share link.",
        variant: "error",
      })
    }
  }, [closeDropdown, selectedNetwork, showToast])

  const networkFormTitle = networkFormMode?.type === "edit" ? "Edit network" : "Add network"
  const networkFormSubmitLabel = networkFormMode?.type === "edit" ? "Save" : "Add"

  useEffect(() => {
    if (!open) {
      return
    }

    const handlePointerDown = (event: PointerEvent) => {
      if (!dropdownRef.current?.contains(event.target as Node)) {
        closeDropdown()
      }
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeDropdown()
      }
    }

    document.addEventListener("pointerdown", handlePointerDown)
    document.addEventListener("keydown", handleKeyDown)
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown)
      document.removeEventListener("keydown", handleKeyDown)
    }
  }, [closeDropdown, open])

  return (
    <div className={styles.networkDropdown} ref={dropdownRef}>
      <button
        type="button"
        className={styles.networkTrigger}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => {
          if (open) {
            closeDropdown()
          } else {
            setOpen(true)
          }
        }}
      >
        <span className={styles.networkTriggerLabel}>{selectedNetwork.label}</span>
        <ChevronDown size={14} aria-hidden="true" />
      </button>
      {open && (
        <div className={styles.networkMenu} data-adding={isNetworkFormOpen}>
          <div className={styles.networkOptions} role="menu" aria-label="Explorer network">
            {networks.map(network => {
              const networkId = network.id
              const active = network.id === value
              const deletable = isCustomNetworkId(networkId)
              return deletable ? (
                <div key={networkId} className={styles.networkOptionRow} data-active={active}>
                  <button
                    type="button"
                    className={styles.networkOptionSelect}
                    role="menuitemradio"
                    aria-checked={active}
                    onClick={() => {
                      onChange(networkId)
                      closeDropdown()
                    }}
                  >
                    <span className={styles.networkOptionText}>
                      <span className={styles.networkOptionLabel}>{network.label}</span>
                      <span className={styles.networkOptionMeta}>{apiHostname(network)}</span>
                    </span>
                    {active && <Check size={16} aria-hidden="true" />}
                  </button>
                  <span className={styles.networkOptionActions}>
                    <button
                      type="button"
                      className={styles.networkIconButton}
                      aria-label={`Edit ${network.label} network`}
                      onClick={() => startEditNetwork(network)}
                    >
                      <Edit2 size={14} aria-hidden="true" />
                    </button>
                    <button
                      type="button"
                      className={styles.networkIconButton}
                      aria-label={`Delete ${network.label} network`}
                      onClick={() => {
                        onDeleteNetwork(networkId)
                        showToast({
                          description: "Network removed.",
                          variant: "success",
                        })
                      }}
                    >
                      <Trash2 size={14} aria-hidden="true" />
                    </button>
                  </span>
                </div>
              ) : (
                <button
                  key={networkId}
                  type="button"
                  className={styles.networkOption}
                  role="menuitemradio"
                  aria-checked={active}
                  data-active={active}
                  onClick={() => {
                    onChange(networkId)
                    closeDropdown()
                  }}
                >
                  <span className={styles.networkOptionText}>
                    <span className={styles.networkOptionLabel}>{network.label}</span>
                    <span className={styles.networkOptionMeta}>{apiHostname(network)}</span>
                  </span>
                  {active && <Check size={16} aria-hidden="true" />}
                </button>
              )
            })}
          </div>
          <div className={styles.networkMenuDivider} />
          <div className={styles.networkAddButtonFrame} data-hidden={isNetworkFormOpen}>
            <button
              type="button"
              className={styles.networkShareOption}
              onClick={() => {
                void handleCopyShareLink()
              }}
            >
              <span className={styles.networkAddOptionInner}>
                <Share2 size={15} aria-hidden="true" />
                Copy link with network
              </span>
            </button>
            <button
              type="button"
              className={styles.networkAddOption}
              disabled={isNetworkFormOpen}
              onClick={startAddNetwork}
            >
              <span className={styles.networkAddOptionInner}>
                <Plus size={15} aria-hidden="true" />
                Add network
              </span>
            </button>
          </div>
          <div
            className={styles.networkAddPanel}
            data-open={isNetworkFormOpen}
            aria-hidden={!isNetworkFormOpen}
          >
            <div className={styles.networkAddPanelInner}>
              <form
                className={styles.networkAddForm}
                onSubmit={event => {
                  event.preventDefault()
                  handleSaveCustomNetwork()
                }}
              >
                <div className={styles.networkFormTitle}>{networkFormTitle}</div>
                <label className={styles.networkField}>
                  <span className={styles.networkFieldLabel}>Name</span>
                  <input
                    className={styles.networkInput}
                    value={customName}
                    onChange={event => setCustomName(event.target.value)}
                    placeholder={DEFAULT_CUSTOM_NETWORK_NAME}
                    disabled={!isNetworkFormOpen}
                  />
                </label>
                <label className={styles.networkField}>
                  <span className={styles.networkFieldLabel}>V2 endpoint</span>
                  <input
                    className={styles.networkInput}
                    value={customV2Url}
                    onChange={event => setCustomV2Url(event.target.value)}
                    placeholder="https://example.com/api/v2"
                    disabled={!isNetworkFormOpen}
                    required
                  />
                </label>
                <label className={styles.networkField}>
                  <span className={styles.networkFieldLabel}>V3 endpoint</span>
                  <input
                    className={styles.networkInput}
                    value={customV3Url}
                    onChange={event => setCustomV3Url(event.target.value)}
                    placeholder="https://example.com/api/v3"
                    disabled={!isNetworkFormOpen}
                    required
                  />
                </label>
                <label className={styles.networkField}>
                  <span className={styles.networkFieldLabel}>API key</span>
                  <input
                    type="password"
                    className={styles.networkInput}
                    value={customApiKey}
                    onChange={event => setCustomApiKey(event.target.value)}
                    placeholder="Optional Toncenter API key"
                    autoComplete="off"
                    disabled={!isNetworkFormOpen}
                  />
                </label>
                <label className={styles.networkCheckboxRow}>
                  <input
                    type="checkbox"
                    checked={customTestOnly}
                    disabled={!isNetworkFormOpen}
                    onChange={event => setCustomTestOnly(event.target.checked)}
                  />
                  <span className={styles.networkCheckboxText}>
                    <span className={styles.networkCheckboxLabel}>Testnet addresses</span>
                    <span className={styles.networkCheckboxDescription}>
                      Copy addresses and open links as testnet addresses.
                    </span>
                  </span>
                </label>
                <label className={styles.networkCheckboxRow}>
                  <input
                    type="checkbox"
                    checked={customSupportsActions}
                    disabled={!isNetworkFormOpen}
                    onChange={event => setCustomSupportsActions(event.target.checked)}
                  />
                  <span className={styles.networkCheckboxText}>
                    <span className={styles.networkCheckboxLabel}>Support actions</span>
                    <span className={styles.networkCheckboxDescription}>
                      Enable Toncenter /actions history for this network.
                    </span>
                  </span>
                </label>
                <div className={styles.networkFormActions}>
                  <button
                    type="button"
                    className={styles.networkCancelButton}
                    disabled={!isNetworkFormOpen}
                    onClick={resetNetworkForm}
                  >
                    Cancel
                  </button>
                  <button
                    type="submit"
                    className={styles.networkSubmitButton}
                    disabled={!isNetworkFormOpen}
                  >
                    {networkFormSubmitLabel}
                  </button>
                </div>
              </form>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

function isCustomNetworkId(id: SelectableExplorerNetworkId): id is CustomExplorerNetworkId {
  return id.startsWith("custom:")
}

const ExplorerHeaderFrame: FC<{readonly children: ReactNode}> = ({children}) => {
  const location = useLocation()
  const isHomePage = location.pathname === "/"
  const headerClassName = isHomePage ? `${styles.header} ${styles.headerHome}` : styles.header

  return <header className={headerClassName}>{children}</header>
}

export const ExplorerApp: FC = () => {
  const [theme, setTheme] = useState<ThemeMode>(readInitialTheme)
  const [networkState, setNetworkState] = useState<ExplorerNetworkState>(
    readInitialExplorerNetworkState,
  )
  const selectableNetworks = useMemo<readonly SelectableExplorerNetwork[]>(
    () => [...EXPLORER_NETWORKS, ...networkState.customNetworks],
    [networkState.customNetworks],
  )
  const networkId = networkState.selectedNetworkId
  const networkConfig =
    selectableNetworks.find(network => network.id === networkId) ?? EXPLORER_API_CONFIGS.mainnet
  const client = useMemo(
    () =>
      new TonClient({
        v2BaseUrl: networkConfig.api.v2BaseUrl,
        v3BaseUrl: networkConfig.api.v3BaseUrl,
        addressNameBaseUrl: "",
        localnetControlEnabled: false,
        toncenterApiCompatible: true,
        toncenterApiKey: networkConfig.api.toncenterApiKey,
      }),
    [networkConfig],
  )
  const metadataRegistry = useMemo(
    () =>
      new CompositeMetadataRegistry([
        new BrowserMetadataRegistry(`actonscan:${networkConfig.id}`),
        new BundledAbiRegistry(getBundledCompilerAbis),
        new VerifierMetadataRegistry(),
      ]),
    [networkConfig.id],
  )
  const handleNetworkChange = useCallback((selectedNetworkId: SelectableExplorerNetworkId) => {
    setNetworkState(current => ({...current, selectedNetworkId}))
  }, [])
  const handleAddNetwork = useCallback((network: SelectableExplorerNetwork) => {
    setNetworkState(current => {
      const customNetworks = upsertCustomNetwork(current.customNetworks, network)
      return {
        customNetworks,
        selectedNetworkId: network.id,
      }
    })
  }, [])
  const handleEditNetwork = useCallback(
    (previousNetworkId: CustomExplorerNetworkId, network: SelectableExplorerNetwork) => {
      setNetworkState(current => {
        const customNetworks = [
          ...current.customNetworks.filter(
            customNetwork =>
              customNetwork.id !== previousNetworkId && customNetwork.id !== network.id,
          ),
          network,
        ]
        return {
          customNetworks,
          selectedNetworkId:
            current.selectedNetworkId === previousNetworkId
              ? network.id
              : current.selectedNetworkId,
        }
      })
    },
    [],
  )
  const handleDeleteNetwork = useCallback((deletedNetworkId: CustomExplorerNetworkId) => {
    setNetworkState(current => {
      const customNetworks = current.customNetworks.filter(
        customNetwork => customNetwork.id !== deletedNetworkId,
      )
      return {
        customNetworks,
        selectedNetworkId:
          current.selectedNetworkId === deletedNetworkId ? "mainnet" : current.selectedNetworkId,
      }
    })
  }, [])

  useLayoutEffect(() => {
    document.documentElement.classList.toggle("dark-theme", theme === "dark")
    document.body.classList.toggle("dark-mode", theme === "dark")
    document.body.classList.toggle("light-mode", theme !== "dark")
    localStorage.setItem("explorerTheme", theme)
  }, [theme])

  useEffect(() => {
    localStorage.setItem(EXPLORER_NETWORK_STORAGE_KEY, networkId)
  }, [networkId])

  useEffect(() => {
    localStorage.setItem(
      EXPLORER_CUSTOM_NETWORKS_STORAGE_KEY,
      JSON.stringify(networkState.customNetworks.map(serializeCustomExplorerNetwork)),
    )
  }, [networkState.customNetworks])

  useEffect(() => {
    if (readSharedCustomExplorerNetwork()) {
      removeSharedNetworkSearchParamsFromUrl()
    }
  }, [])

  const toggleTheme = () => setTheme(current => (current === "dark" ? "light" : "dark"))

  return (
    <BrowserRouter>
      <ToastProvider>
        <StaticNetworkInfoProvider network={networkConfig}>
          <ExplorerRoutesProvider basePath="">
            <MetadataRegistryProvider registry={metadataRegistry}>
              <AddressBookProvider>
                <div className={styles.appShell}>
                  <ExplorerHeaderFrame>
                    <div className={styles.headerInner}>
                      <div className={styles.headerPrimary}>
                        <Link className={styles.brand} to="/">
                          <img
                            className={styles.brandIcon}
                            src={actonScanLogo}
                            alt=""
                            aria-hidden="true"
                          />
                          <span className={styles.brandText}>actonscan</span>
                        </Link>
                        <nav className={styles.nav} aria-label="Explorer navigation">
                          <Link className={styles.navLink} to="/blocks">
                            Blocks
                          </Link>
                          <Link className={styles.navLink} to="/abi">
                            ABI
                          </Link>
                          <Link className={styles.navLink} to="/sources">
                            Sources
                          </Link>
                        </nav>
                      </div>
                      <ExplorerSearch className={styles.headerSearch} variant="header" />
                      <div className={styles.headerActions}>
                        <NetworkDropdown
                          networks={selectableNetworks}
                          value={networkId}
                          onChange={handleNetworkChange}
                          onAddNetwork={handleAddNetwork}
                          onEditNetwork={handleEditNetwork}
                          onDeleteNetwork={handleDeleteNetwork}
                        />
                        <ThemeSwitch
                          theme={theme}
                          onToggleTheme={toggleTheme}
                          aria-label={theme === "dark" ? "Use light theme" : "Use dark theme"}
                        />
                        <a
                          className={styles.githubButton}
                          href="https://github.com/ton-blockchain/acton"
                          target="_blank"
                          rel="noreferrer"
                          title="Open GitHub"
                          aria-label="Open GitHub"
                        >
                          <Github size={18} />
                        </a>
                      </div>
                    </div>
                  </ExplorerHeaderFrame>
                  <main className={styles.main}>
                    <Routes>
                      <Route path="/" element={<ExplorerIndexPage />} />
                      <Route path="/blocks" element={<BlocksPage client={client} />} />
                      <Route path="/abi" element={<AbiCatalogPage />} />
                      <Route path="/abi/:slug" element={<AbiDetailsPage />} />
                      <Route path="/sources" element={<SourceCatalogPage />} />
                      <Route
                        path="/block/:workchain/:shard/:seqno"
                        element={<BlockDetailsPage client={client} />}
                      />
                      <Route path="/address/:address" element={<AccountPage client={client} />} />
                      <Route
                        path="/tx/:hash/trace"
                        element={<TransactionPage client={client} openRetraceOnLoad />}
                      />
                      <Route path="/tx/:hash" element={<TransactionPage client={client} />} />
                      <Route path="*" element={<Navigate to="/" replace />} />
                    </Routes>
                  </main>
                </div>
              </AddressBookProvider>
            </MetadataRegistryProvider>
          </ExplorerRoutesProvider>
        </StaticNetworkInfoProvider>
      </ToastProvider>
    </BrowserRouter>
  )
}

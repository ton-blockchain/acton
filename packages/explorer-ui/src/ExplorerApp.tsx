import {Checkbox, Input, Popover, ThemeSwitch, ToastProvider, useToast} from "@acton/ui"
import {
  createVerifierApi,
  readVerifiedContractsPage,
  StatisticsPage,
  verifiedContractsPageSearch,
  VerifiedContractPage,
  VerifiedContractsPage,
} from "@acton/verifier-ui"
import {
  Check,
  ChevronDown,
  Edit2,
  Github,
  Menu,
  Plus,
  Search,
  Share2,
  Star,
  Trash2,
  X,
} from "lucide-react"
import {useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState} from "react"
import type {FC, ReactNode} from "react"
import {
  BrowserRouter,
  Link,
  Navigate,
  Route,
  Routes,
  useLocation,
  useNavigate,
  useParams,
} from "react-router"

import {TonClient} from "@acton/explorer-core/api/client"
import {getBundledCompilerAbis} from "@acton/explorer-core/api/compilerAbiCatalog"
import {AddressBookProvider} from "@acton/explorer-core/hooks/useAddressBook"
import {ExplorerRoutesProvider} from "@acton/explorer-core/hooks/useExplorerRoutes"
import {StaticNetworkInfoProvider} from "@acton/explorer-core/hooks/StaticNetworkInfoProvider"
import {BrowserMetadataRegistry} from "@acton/explorer-core/metadata/browserRegistry"
import {BundledAbiRegistry} from "@acton/explorer-core/metadata/bundledAbiRegistry"
import {CompositeMetadataRegistry} from "@acton/explorer-core/metadata/compositeRegistry"
import {MetadataRegistryProvider} from "@acton/explorer-core/metadata/MetadataRegistryProvider"
import {VerifierMetadataRegistry} from "@acton/explorer-core/metadata/verifierRegistry"
import type {
  CustomExplorerNetworkId,
  ExplorerApiConfig,
  ExplorerNetworkInfo,
} from "@acton/explorer-core/hooks/useNetworkInfo"
import {TokenCatalogPage} from "@acton/explorer-core/pages/TokenCatalogPage"
import {BlockDetailsPage, BlocksPage} from "@acton/explorer-core/pages/BlocksPage"
import {AccountPage} from "@acton/explorer-core/pages/AccountPage"
import {ExplorerSearch} from "@acton/explorer-core/components/ExplorerSearch"
import {ExplorerDocumentTitle} from "@acton/explorer-core/components/ExplorerDocumentTitle"
import {ExplorerIndexPage} from "@acton/explorer-core/pages/ExplorerIndexPage"
import {FavoriteAccountsPage} from "@acton/explorer-core/pages/FavoriteAccountsPage"
import {SuspendedAddressesPage} from "@acton/explorer-core/pages/SuspendedAddressesPage"
import {TransactionPage} from "@acton/explorer-core/pages/TransactionPage"
import "@acton/ui/styles/tokens.css"
import "@acton/explorer-core/styles.css"
import actonScanCustomLogo from "./assets/acton-scan-custom-logo-dark.svg"
import actonScanLogo from "./assets/acton-scan-logo-dark.svg"
import actonScanTestnetLogo from "./assets/acton-scan-testnet-logo-dark.svg"
import {DeveloperExplorerBanner} from "./components/DeveloperExplorerBanner"
import {EXPLORER_NETWORK_QUERY_PARAM, explorerNetworkSearch} from "./explorerNetworkUrl"
import {FaucetPage} from "./faucet/FaucetPage"
import {AbiCatalogPage, AbiDetailsPage} from "./pages/abi-pages"
import {SourceCatalogPage} from "./pages/SourceCatalogPage"
import {CellInspectorExplorerPage, EmulateExplorerPage} from "./pages/explorer-tool-pages"
import {loadNetworkTps} from "./actonscanBackend"
import styles from "./ExplorerApp.module.css"

type BuiltinSelectableExplorerNetworkId = "mainnet" | "testnet"
type SelectableExplorerNetworkId = BuiltinSelectableExplorerNetworkId | CustomExplorerNetworkId
type SelectableExplorerNetwork = ExplorerNetworkInfo & {
  readonly id: SelectableExplorerNetworkId
  readonly api: ExplorerApiConfig
}
type ExplorerNetworkKind = BuiltinSelectableExplorerNetworkId | "custom"

const EXPLORER_NETWORK_LOGOS: Record<ExplorerNetworkKind, string> = {
  mainnet: actonScanLogo,
  testnet: actonScanTestnetLogo,
  custom: actonScanCustomLogo,
}

type NetworkFormMode =
  | {readonly type: "add"}
  | {readonly type: "edit"; readonly networkId: CustomExplorerNetworkId}

const EXPLORER_NETWORK_STORAGE_KEY = "explorerNetwork"
const EXPLORER_CUSTOM_NETWORKS_STORAGE_KEY = "explorerCustomNetworks"
const ACTON_VERIFIER_API = createVerifierApi({
  baseUrl: "https://verifier.acton.monster/api/v1",
})
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
  const requestedNetwork = new URLSearchParams(globalThis.location.search).get(
    EXPLORER_NETWORK_QUERY_PARAM,
  )
  if (requestedNetwork === "mainnet" || requestedNetwork === "testnet") {
    return requestedNetwork
  }

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
        title: networkFormMode.type === "edit" ? "Network updated" : "Network added",
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
        title: "Network link copied",
        description:
          isCustomNetworkId(selectedNetwork.id) && selectedNetwork.api.toncenterApiKey
            ? "API key was not included in the shared link."
            : undefined,
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
                          title: "Network removed",
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
                <label className={styles.networkField} htmlFor="custom-network-name">
                  <span className={styles.networkFieldLabel}>Name</span>
                  <Input
                    id="custom-network-name"
                    size="sm"
                    className={styles.networkInput}
                    value={customName}
                    onChange={event => setCustomName(event.target.value)}
                    placeholder={DEFAULT_CUSTOM_NETWORK_NAME}
                    disabled={!isNetworkFormOpen}
                  />
                </label>
                <label className={styles.networkField} htmlFor="custom-network-v2-endpoint">
                  <span className={styles.networkFieldLabel}>V2 endpoint</span>
                  <Input
                    id="custom-network-v2-endpoint"
                    size="sm"
                    className={styles.networkInput}
                    value={customV2Url}
                    onChange={event => setCustomV2Url(event.target.value)}
                    placeholder="https://example.com/api/v2"
                    disabled={!isNetworkFormOpen}
                    required
                  />
                </label>
                <label className={styles.networkField} htmlFor="custom-network-v3-endpoint">
                  <span className={styles.networkFieldLabel}>V3 endpoint</span>
                  <Input
                    id="custom-network-v3-endpoint"
                    size="sm"
                    className={styles.networkInput}
                    value={customV3Url}
                    onChange={event => setCustomV3Url(event.target.value)}
                    placeholder="https://example.com/api/v3"
                    disabled={!isNetworkFormOpen}
                    required
                  />
                </label>
                <label className={styles.networkField} htmlFor="custom-network-api-key">
                  <span className={styles.networkFieldLabel}>API key</span>
                  <Input
                    id="custom-network-api-key"
                    size="sm"
                    type="password"
                    className={styles.networkInput}
                    value={customApiKey}
                    onChange={event => setCustomApiKey(event.target.value)}
                    placeholder="Optional Toncenter API key"
                    autoComplete="off"
                    disabled={!isNetworkFormOpen}
                  />
                </label>
                <Checkbox
                  className={styles.networkCheckbox}
                  label="Testnet addresses"
                  description="Copy addresses and open links as testnet addresses."
                  checked={customTestOnly}
                  disabled={!isNetworkFormOpen}
                  onChange={event => setCustomTestOnly(event.currentTarget.checked)}
                />
                <Checkbox
                  className={styles.networkCheckbox}
                  label="Support actions"
                  description="Enable Toncenter /actions history for this network."
                  checked={customSupportsActions}
                  disabled={!isNetworkFormOpen}
                  onChange={event => setCustomSupportsActions(event.currentTarget.checked)}
                />
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

const ExplorerNetworkUrlSync: FC<{
  readonly networkId: SelectableExplorerNetworkId
}> = ({networkId}) => {
  const location = useLocation()
  const navigate = useNavigate()

  useEffect(() => {
    const search = explorerNetworkSearch(location.search, networkId)
    if (search === new URLSearchParams(location.search).toString()) return

    void navigate(
      {
        pathname: location.pathname,
        search,
        hash: location.hash,
      },
      {replace: true},
    )
  }, [location.hash, location.pathname, location.search, navigate, networkId])

  return null
}

const MobileHeaderRouteSync: FC<{readonly onNavigate: () => void}> = ({onNavigate}) => {
  const location = useLocation()
  const routeIdentity = `${location.pathname}${location.search}${location.hash}`

  useEffect(() => {
    if (routeIdentity.length > 0) {
      onNavigate()
    }
  }, [onNavigate, routeIdentity])

  return null
}

const DesktopMoreMenu: FC = () => {
  const [open, setOpen] = useState(false)
  const closeMenu = () => setOpen(false)

  useEffect(() => {
    const mobileHeaderQuery = globalThis.matchMedia("(max-width: 640px)")
    const closeOnMobile = (event: MediaQueryListEvent) => {
      if (event.matches) {
        setOpen(false)
      }
    }

    mobileHeaderQuery.addEventListener("change", closeOnMobile)
    return () => mobileHeaderQuery.removeEventListener("change", closeOnMobile)
  }, [])

  return (
    <Popover
      interaction="click"
      placement="bottom"
      open={open}
      onOpenChange={setOpen}
      triggerAsChild
      contentClassName={styles.desktopMorePopover}
      content={
        <nav className={styles.desktopMoreMenu} aria-label="More explorer navigation">
          <Link className={styles.desktopMoreItem} to="/tokens" onClick={closeMenu}>
            <span className={styles.desktopMoreItemCopy}>
              <span className={styles.desktopMoreItemTitle}>Tokens</span>
              <span className={styles.desktopMoreItemDescription}>Discover active tokens</span>
            </span>
          </Link>
          <Link className={styles.desktopMoreItem} to="/faucet" onClick={closeMenu}>
            <span className={styles.desktopMoreItemCopy}>
              <span className={styles.desktopMoreItemTitle}>Faucet</span>
              <span className={styles.desktopMoreItemDescription}>Get GRAM for TON Testnet</span>
            </span>
          </Link>
          <Link className={styles.desktopMoreItem} to="/cell" onClick={closeMenu}>
            <span className={styles.desktopMoreItemCopy}>
              <span className={styles.desktopMoreItemTitle}>Cell Inspector</span>
              <span className={styles.desktopMoreItemDescription}>
                Inspect and decode TON cells
              </span>
            </span>
          </Link>
          <Link className={styles.desktopMoreItem} to="/emulate" onClick={closeMenu}>
            <span className={styles.desktopMoreItemCopy}>
              <span className={styles.desktopMoreItemTitle}>Emulator</span>
              <span className={styles.desktopMoreItemDescription}>
                Emulate transactions locally
              </span>
            </span>
          </Link>
          <Link className={styles.desktopMoreItem} to="/verified" onClick={closeMenu}>
            <span className={styles.desktopMoreItemCopy}>
              <span className={styles.desktopMoreItemTitle}>Verified contracts</span>
              <span className={styles.desktopMoreItemDescription}>
                Browse verified on-chain source code
              </span>
            </span>
          </Link>
        </nav>
      }
    >
      <button
        type="button"
        className={`${styles.navLink} ${styles.desktopMoreTrigger}`}
        aria-label="Open more navigation"
        aria-expanded={open}
      >
        <span aria-hidden="true">•••</span>
      </button>
    </Popover>
  )
}

interface VerifiedContractsViewState {
  readonly page: number
  readonly scrollY: number
}

interface VerifiedContractsRouteState {
  readonly verifiedContractsView?: VerifiedContractsViewState
}

function readVerifiedContractsViewState(value: unknown): VerifiedContractsViewState | undefined {
  if (typeof value !== "object" || value === null || !("verifiedContractsView" in value)) {
    return undefined
  }

  const view = (value as VerifiedContractsRouteState).verifiedContractsView
  if (
    !view ||
    !Number.isFinite(view.page) ||
    view.page < 0 ||
    !Number.isFinite(view.scrollY) ||
    view.scrollY < 0
  ) {
    return undefined
  }

  return {
    page: Math.trunc(view.page),
    scrollY: view.scrollY,
  }
}

const VerifiedContractsRoute: FC = () => {
  const location = useLocation()
  const navigate = useNavigate()
  const restoration = readVerifiedContractsViewState(location.state)
  const initialPage = readVerifiedContractsPage(location.search)
  const currentPageRef = useRef(initialPage)
  const restoredScrollRef = useRef(false)

  useEffect(() => {
    currentPageRef.current = initialPage
  }, [initialPage])

  return (
    <VerifiedContractsPage
      api={ACTON_VERIFIER_API}
      getContractHref={item => `/verified/${encodeURIComponent(item.code_hash)}`}
      statisticsHref="/verified/statistics"
      onOpenStatistics={() => void navigate("/verified/statistics")}
      page={initialPage}
      onPageChange={page => {
        currentPageRef.current = page
        const nextSearch = verifiedContractsPageSearch(location.search, page)
        if (nextSearch !== location.search.slice(1)) {
          void navigate(
            {
              pathname: location.pathname,
              search: nextSearch ? `?${nextSearch}` : "",
              hash: location.hash,
            },
            {replace: true},
          )
        }
      }}
      onContentReady={() => {
        if (
          restoredScrollRef.current ||
          restoration === undefined ||
          restoration.page !== initialPage
        ) {
          return
        }

        restoredScrollRef.current = true
        globalThis.requestAnimationFrame(() => {
          globalThis.scrollTo({top: restoration.scrollY, behavior: "auto"})
        })
      }}
      onOpenContract={item => {
        void navigate(
          {
            pathname: location.pathname,
            search: location.search,
            hash: location.hash,
          },
          {
            replace: true,
            state: {
              verifiedContractsView: {
                page: currentPageRef.current,
                scrollY: globalThis.scrollY,
              },
            } satisfies VerifiedContractsRouteState,
          },
        )
        void navigate(`/verified/${encodeURIComponent(item.code_hash)}`)
      }}
    />
  )
}

const VerifiedContractRoute: FC = () => {
  const {target = ""} = useParams<{target: string}>()
  const location = useLocation()
  const navigate = useNavigate()
  const selectedSourcePath = new URLSearchParams(location.search).get("file") ?? undefined

  return (
    <VerifiedContractPage
      api={ACTON_VERIFIER_API}
      target={target}
      selectedSourcePath={selectedSourcePath}
      onSelectedSourcePathChange={path => {
        const params = new URLSearchParams(location.search)
        params.set("file", path)
        void navigate(`${location.pathname}?${params.toString()}`, {replace: true})
      }}
    />
  )
}

export const ExplorerApp: FC = () => {
  const [networkState, setNetworkState] = useState<ExplorerNetworkState>(
    readInitialExplorerNetworkState,
  )
  const [mobileHeaderPanel, setMobileHeaderPanel] = useState<"navigation" | "search">()
  const mobileNavigationRef = useRef<HTMLDivElement>(null)
  const mobileSearchRef = useRef<HTMLDivElement>(null)
  const mobileNavigationOpen = mobileHeaderPanel === "navigation"
  const mobileSearchOpen = mobileHeaderPanel === "search"
  const closeMobileHeaderPanels = useCallback(() => setMobileHeaderPanel(undefined), [])
  const selectableNetworks = useMemo<readonly SelectableExplorerNetwork[]>(
    () => [...EXPLORER_NETWORKS, ...networkState.customNetworks],
    [networkState.customNetworks],
  )
  const networkId = networkState.selectedNetworkId
  const networkConfig =
    selectableNetworks.find(network => network.id === networkId) ?? EXPLORER_API_CONFIGS.mainnet
  const networkKind: ExplorerNetworkKind = isCustomNetworkId(networkId) ? "custom" : networkId
  const brandLogo = EXPLORER_NETWORK_LOGOS[networkKind]
  const client = useMemo(
    () =>
      new TonClient({
        v2BaseUrl: networkConfig.api.v2BaseUrl,
        v3BaseUrl: networkConfig.api.v3BaseUrl,
        toncenterProxyV2BaseUrl:
          networkKind === "custom" ? undefined : `/api/toncenter/${networkKind}/v2`,
        toncenterProxyV3BaseUrl:
          networkKind === "custom" ? undefined : `/api/toncenter/${networkKind}/v3`,
        addressNameBaseUrl: "",
        localnetControlEnabled: false,
        toncenterApiCompatible: true,
        toncenterApiKey: networkConfig.api.toncenterApiKey,
      }),
    [networkConfig, networkKind],
  )
  const testnetClient = useMemo(
    () =>
      new TonClient({
        v2BaseUrl: EXPLORER_API_CONFIGS.testnet.api.v2BaseUrl,
        v3BaseUrl: EXPLORER_API_CONFIGS.testnet.api.v3BaseUrl,
        addressNameBaseUrl: "",
        localnetControlEnabled: false,
        toncenterApiCompatible: true,
        toncenterApiKey: EXPLORER_API_CONFIGS.testnet.api.toncenterApiKey,
      }),
    [],
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
    document.querySelector<HTMLLinkElement>('link[rel="icon"]')?.setAttribute("href", brandLogo)
  }, [brandLogo])

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

  useEffect(() => {
    if (!mobileHeaderPanel) return

    const handlePointerDown = (event: PointerEvent) => {
      if (
        event.target instanceof Node &&
        (mobileNavigationRef.current?.contains(event.target) ||
          mobileSearchRef.current?.contains(event.target))
      ) {
        return
      }
      closeMobileHeaderPanels()
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeMobileHeaderPanels()
      }
    }

    document.addEventListener("pointerdown", handlePointerDown, true)
    document.addEventListener("keydown", handleKeyDown)
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown, true)
      document.removeEventListener("keydown", handleKeyDown)
    }
  }, [closeMobileHeaderPanels, mobileHeaderPanel])

  return (
    <BrowserRouter>
      <ExplorerNetworkUrlSync networkId={networkId} />
      <MobileHeaderRouteSync onNavigate={closeMobileHeaderPanels} />
      <ToastProvider>
        <StaticNetworkInfoProvider network={networkConfig}>
          <ExplorerRoutesProvider basePath="">
            <MetadataRegistryProvider registry={metadataRegistry}>
              <AddressBookProvider>
                <ExplorerDocumentTitle
                  productName={
                    networkId === "mainnet" ? "actonscan" : `actonscan ${networkConfig.label}`
                  }
                />
                <div className={styles.appShell} data-network-kind={networkKind}>
                  <DeveloperExplorerBanner />
                  <ExplorerHeaderFrame>
                    <div className={styles.headerInner}>
                      <div className={styles.headerPrimary}>
                        <Link className={styles.brand} to="/">
                          <img
                            className={styles.brandIcon}
                            src={brandLogo}
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
                          <DesktopMoreMenu />
                        </nav>
                      </div>
                      <ExplorerSearch
                        className={styles.headerSearch}
                        client={client}
                        variant="header"
                      />
                      <div className={styles.headerActions}>
                        <NetworkDropdown
                          networks={selectableNetworks}
                          value={networkId}
                          onChange={handleNetworkChange}
                          onAddNetwork={handleAddNetwork}
                          onEditNetwork={handleEditNetwork}
                          onDeleteNetwork={handleDeleteNetwork}
                        />
                        <div className={styles.mobileSearchRoot} ref={mobileSearchRef}>
                          <button
                            type="button"
                            className={styles.headerIconButton}
                            aria-label={mobileSearchOpen ? "Close search" : "Open search"}
                            aria-controls="explorer-mobile-search"
                            aria-expanded={mobileSearchOpen}
                            onClick={() =>
                              setMobileHeaderPanel(current =>
                                current === "search" ? undefined : "search",
                              )
                            }
                          >
                            {mobileSearchOpen ? <X size={18} /> : <Search size={18} />}
                          </button>
                          {mobileSearchOpen && (
                            <div id="explorer-mobile-search" className={styles.mobileSearchPanel}>
                              <ExplorerSearch
                                autoFocus
                                className={styles.mobileSearch}
                                client={client}
                                variant="header"
                              />
                            </div>
                          )}
                        </div>
                        <Link
                          className={`${styles.headerIconButton} ${styles.desktopHeaderAction}`}
                          to="/favorites"
                          title="Favorites"
                          aria-label="Favorites"
                        >
                          <Star size={18} />
                        </Link>
                        <span className={styles.desktopHeaderAction}>
                          <ThemeSwitch />
                        </span>
                        <a
                          className={`${styles.headerIconButton} ${styles.desktopHeaderAction}`}
                          href="https://github.com/ton-blockchain/acton"
                          target="_blank"
                          rel="noreferrer"
                          title="Open GitHub"
                          aria-label="Open GitHub"
                        >
                          <Github size={18} />
                        </a>
                        <div className={styles.mobileNavigationRoot} ref={mobileNavigationRef}>
                          <button
                            type="button"
                            className={styles.headerIconButton}
                            aria-label={
                              mobileNavigationOpen ? "Close navigation" : "Open navigation"
                            }
                            aria-controls="explorer-mobile-navigation"
                            aria-expanded={mobileNavigationOpen}
                            onClick={() =>
                              setMobileHeaderPanel(current =>
                                current === "navigation" ? undefined : "navigation",
                              )
                            }
                          >
                            {mobileNavigationOpen ? <X size={18} /> : <Menu size={18} />}
                          </button>
                          {mobileNavigationOpen && (
                            <nav
                              id="explorer-mobile-navigation"
                              className={styles.mobileNavigation}
                              aria-label="Mobile explorer navigation"
                            >
                              <Link to="/blocks" onClick={closeMobileHeaderPanels}>
                                Blocks
                              </Link>
                              <Link to="/tokens" onClick={closeMobileHeaderPanels}>
                                Tokens
                              </Link>
                              {networkId === "mainnet" && (
                                <Link to="/suspended" onClick={closeMobileHeaderPanels}>
                                  Suspended addresses
                                </Link>
                              )}
                              <Link to="/abi" onClick={closeMobileHeaderPanels}>
                                ABI
                              </Link>
                              <Link to="/sources" onClick={closeMobileHeaderPanels}>
                                Sources
                              </Link>
                              <Link to="/faucet" onClick={closeMobileHeaderPanels}>
                                Faucet
                              </Link>
                              <Link to="/verified" onClick={closeMobileHeaderPanels}>
                                Verified contracts
                              </Link>
                              <Link to="/cell" onClick={closeMobileHeaderPanels}>
                                Cell Inspector
                              </Link>
                              <Link to="/emulate" onClick={closeMobileHeaderPanels}>
                                Emulator
                              </Link>
                              <Link to="/favorites" onClick={closeMobileHeaderPanels}>
                                <span>Favorites</span>
                                <Star size={17} aria-hidden="true" />
                              </Link>
                              <a
                                href="https://github.com/ton-blockchain/acton"
                                target="_blank"
                                rel="noreferrer"
                                onClick={closeMobileHeaderPanels}
                              >
                                <span>GitHub</span>
                                <Github size={17} aria-hidden="true" />
                              </a>
                              <div className={styles.mobileThemeRow}>
                                <span>Appearance</span>
                                <ThemeSwitch />
                              </div>
                            </nav>
                          )}
                        </div>
                      </div>
                    </div>
                  </ExplorerHeaderFrame>
                  <main className={styles.main}>
                    <Routes>
                      <Route
                        path="/"
                        element={
                          <ExplorerIndexPage
                            client={client}
                            faucetPath={networkId === "testnet" ? "/faucet" : undefined}
                            fillAvailableHeight
                          />
                        }
                      />
                      <Route
                        path="/blocks"
                        element={
                          <BlocksPage
                            client={client}
                            loadNetworkTps={networkId === "mainnet" ? loadNetworkTps : undefined}
                          />
                        }
                      />
                      <Route path="/tokens" element={<TokenCatalogPage client={client} />} />
                      <Route path="/abi" element={<AbiCatalogPage />} />
                      <Route path="/abi/:slug" element={<AbiDetailsPage />} />
                      <Route path="/sources" element={<SourceCatalogPage client={client} />} />
                      <Route
                        path="/faucet"
                        element={
                          <FaucetPage
                            isTestnetSelected={networkId === "testnet"}
                            selectedNetworkLabel={networkConfig.label}
                            testnetClient={testnetClient}
                            onSwitchToTestnet={() => handleNetworkChange("testnet")}
                          />
                        }
                      />
                      <Route path="/verified" element={<VerifiedContractsRoute />} />
                      <Route
                        path="/verified/statistics"
                        element={<StatisticsPage api={ACTON_VERIFIER_API} />}
                      />
                      <Route path="/verified/:target" element={<VerifiedContractRoute />} />
                      <Route path="/cell" element={<CellInspectorExplorerPage />} />
                      <Route path="/emulate" element={<EmulateExplorerPage client={client} />} />
                      <Route path="/favorites" element={<FavoriteAccountsPage client={client} />} />
                      <Route
                        path="/suspended"
                        element={<SuspendedAddressesPage client={client} />}
                      />
                      <Route
                        path="/block/last"
                        element={<BlockDetailsPage client={client} latest />}
                      />
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

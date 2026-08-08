import type {CSSProperties, FC, KeyboardEvent as ReactKeyboardEvent} from "react"
import {useCallback, useEffect, useMemo, useRef, useState} from "react"
import type {LucideIcon} from "lucide-react"
import {
  Boxes,
  Cable,
  ChartNoAxesColumn,
  CircleUserRound,
  FileJson,
  Image,
  Search,
  Wallet,
} from "lucide-react"
import {useNavigate} from "react-router"

import type {TonClient} from "@acton/explorer-core/api/client"
import type {JettonMaster, NftItem} from "@acton/explorer-core/api/types"
import {NftImage} from "@acton/explorer-core/components/NftImage"
import {formatAddress, hashToHex, parseAddress} from "@acton/explorer-core/components/utils"
import {useExplorerRoutePaths} from "@acton/explorer-core/hooks/useExplorerRoutePaths"
import {useAddressFormat} from "@acton/explorer-core/hooks/useNetworkInfo"
import {supports, supportsAny} from "../../environmentCapabilities"
import type {EnvironmentCapability} from "../../studioApi"
import {useLocalnetRuntime} from "../LocalnetRuntimeProvider"
import {useLocalnetRoutes} from "../routes"

import {type ApiSearchIndexEntry, loadApiSearchIndex} from "./apiSearchIndex"
import {contentString, matchesQuery} from "./dashboardUtils"
import styles from "./DashboardPage.module.css"
import {shortenMiddle} from "@acton/ui"
import {TOKEN_PLACEHOLDER_IMAGE} from "@acton/explorer-core/components/imageFallbacks"

interface DashboardSearchOverlayProps {
  readonly client: TonClient
  readonly isOpen: boolean
  readonly onClose: () => void
  readonly originStyle: Readonly<CSSProperties>
}

interface SearchAssetsState {
  readonly tokens: readonly JettonMaster[]
  readonly nfts: readonly NftItem[]
  readonly isLoading: boolean
  readonly isLoaded: boolean
  readonly error?: string
}

interface ApiSearchState {
  readonly entries: readonly ApiSearchIndexEntry[]
  readonly isLoading: boolean
  readonly isLoaded: boolean
  readonly error?: string
}

interface SearchResult {
  readonly id: string
  readonly title: string
  readonly description: string
  readonly href: string
  readonly icon: LucideIcon
  readonly workspace?: boolean
  readonly image?: string
  readonly fallbackImage?: string
  readonly collectionName?: string
  readonly isScam?: boolean
  readonly capabilities?: readonly EnvironmentCapability[]
}

interface LoadedSearchAssets {
  readonly tokens: readonly JettonMaster[]
  readonly nfts: readonly NftItem[]
}

const searchAssetsCache = new WeakMap<TonClient, Promise<LoadedSearchAssets>>()

const quickSearchResults: readonly SearchResult[] = [
  {
    id: "quick-home",
    title: "Environment",
    description: "Open the environment home",
    href: "/dashboard",
    icon: CircleUserRound,
    workspace: true,
  },
  {
    id: "quick-explorer",
    title: "Explorer",
    description: "Search any address or transaction",
    href: "/explorer",
    icon: Search,
    capabilities: ["explorer"],
  },
  {
    id: "quick-faucet",
    title: "Faucet",
    description: "Fund an account in this environment",
    href: "/faucet",
    icon: Wallet,
    capabilities: ["gramFaucet", "jettonFaucet"],
  },
  {
    id: "quick-integrate",
    title: "Integrate",
    description: "Connect a project, application or TON-compatible tool",
    href: "/integrate",
    icon: Cable,
    capabilities: ["integration"],
  },
  {
    id: "quick-api-calls",
    title: "API Calls",
    description: "Recent API traffic handled by this environment",
    href: "/api-calls",
    icon: FileJson,
    capabilities: ["apiCalls"],
  },
  {
    id: "quick-tokens",
    title: "Tokens",
    description: "Jettons indexed from this environment",
    href: "/explorer/tokens",
    icon: Boxes,
    capabilities: ["explorer"],
  },
  {
    id: "quick-nfts",
    title: "NFTs",
    description: "NFT items indexed from this environment",
    href: "/explorer/nfts",
    icon: Image,
    capabilities: ["explorer"],
  },
]

export const DashboardSearchOverlay: FC<DashboardSearchOverlayProps> = ({
  client,
  isOpen,
  onClose,
  originStyle,
}) => {
  const [hiddenNftResultIds, setHiddenNftResultIds] = useState<ReadonlySet<string>>(() => new Set())
  const navigate = useNavigate()
  const addressFormat = useAddressFormat()
  const routes = useExplorerRoutePaths()
  const localnetRoutes = useLocalnetRoutes()
  const {environment} = useLocalnetRuntime()
  const [searchQuery, setSearchQuery] = useState("")
  const [searchAssetsState, setSearchAssetsState] = useState<SearchAssetsState>({
    tokens: [],
    nfts: [],
    isLoading: false,
    isLoaded: false,
  })
  const [apiSearchState, setApiSearchState] = useState<ApiSearchState>({
    entries: [],
    isLoading: false,
    isLoaded: false,
  })
  const searchInputRef = useRef<HTMLInputElement>(null)

  const searchResults = useMemo<readonly SearchResult[]>(() => {
    const trimmed = searchQuery.trim()
    if (trimmed.length === 0) {
      return quickSearchResults
        .filter(
          result =>
            result.capabilities === undefined || supportsAny(environment, ...result.capabilities),
        )
        .map(result => ({
          ...result,
          title: result.id === "quick-home" ? (environment?.name ?? result.title) : result.title,
          href: localnetRoutes.path(result.href),
        }))
    }

    const query = trimmed.toLocaleLowerCase()
    const results: SearchResult[] = []
    const transactionHash = hashToHex(trimmed)
    if (transactionHash) {
      results.push({
        id: `transaction-${transactionHash}`,
        title: "Open transaction",
        description: shortenMiddle(transactionHash, {start: 8, end: 8}),
        href: routes.transactionPath(transactionHash),
        icon: ChartNoAxesColumn,
      })
    }

    const address = parseAddress(trimmed)?.toString(addressFormat)
    if (address) {
      results.push({
        id: `address-${address}`,
        title: "Open address",
        description: formatAddress(address, false, addressFormat),
        href: routes.addressPath(address),
        icon: CircleUserRound,
      })
    }

    if (results.length < 12) {
      for (const entry of apiSearchState.entries) {
        if (!supports(environment, entry.capability) || !entry.searchText.includes(query)) {
          continue
        }

        results.push({
          id: entry.id,
          title: entry.title,
          description: entry.description,
          href: localnetRoutes.path(entry.href),
          icon: FileJson,
        })
        if (results.length >= 12) {
          break
        }
      }
    }

    for (const token of searchAssetsState.tokens) {
      if (results.length >= 12) {
        break
      }

      const name = token.jetton_content.name || "Unknown Jetton"
      const symbol = token.jetton_content.symbol || "???"
      const description = token.jetton_content.description
      if (!matchesQuery([name, symbol, description, token.address], query)) {
        continue
      }

      results.push({
        id: `token-${token.address}`,
        title: name,
        description: `Token · ${symbol}`,
        href: routes.addressPath(token.address),
        icon: Boxes,
        image: token.jetton_content.image,
        fallbackImage: TOKEN_PLACEHOLDER_IMAGE,
      })
      if (results.length >= 12) {
        break
      }
    }

    if (results.length < 12) {
      for (const item of searchAssetsState.nfts) {
        const name = contentString(item.content, "name") || "NFT Item"
        const description = contentString(item.content, "description")
        const collectionName = contentString(item.collection?.collection_content, "name")
        if (!matchesQuery([name, description, collectionName, item.address], query)) {
          continue
        }

        results.push({
          id: `nft-${item.address}`,
          title: name,
          description: collectionName ? `NFT · ${collectionName}` : `NFT · #${item.index}`,
          href: routes.addressPath(item.address),
          icon: Image,
          image: contentString(item.content, "image"),
          fallbackImage: TOKEN_PLACEHOLDER_IMAGE,
          collectionName,
          isScam: item.is_scam === true,
        })
        if (results.length >= 12) {
          break
        }
      }
    }

    return results
  }, [
    addressFormat,
    apiSearchState.entries,
    environment,
    localnetRoutes,
    routes,
    searchAssetsState.nfts,
    searchAssetsState.tokens,
    searchQuery,
  ])

  const selectSearchResult = useCallback(
    (result: SearchResult) => {
      onClose()
      setSearchQuery("")
      void navigate(result.href)
    },
    [navigate, onClose],
  )

  const handleSearchKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Escape") {
        event.preventDefault()
        onClose()
        return
      }
      if (event.key === "Enter" && searchResults[0]) {
        event.preventDefault()
        selectSearchResult(searchResults[0])
      }
    },
    [onClose, searchResults, selectSearchResult],
  )

  useEffect(() => {
    if (isOpen) {
      searchInputRef.current?.focus()
    }
  }, [isOpen])

  useEffect(() => {
    if (searchAssetsState.isLoaded) {
      return
    }

    let cancelled = false

    void (async () => {
      setSearchAssetsState(current => ({
        ...current,
        isLoading: true,
        error: undefined,
      }))

      try {
        const {tokens, nfts} = await loadSearchAssets(client)

        if (cancelled) {
          return
        }

        setSearchAssetsState({
          tokens,
          nfts,
          isLoading: false,
          isLoaded: true,
        })
      } catch (error) {
        if (cancelled) {
          return
        }

        setSearchAssetsState({
          tokens: [],
          nfts: [],
          isLoading: false,
          isLoaded: true,
          error: error instanceof Error ? error.message : "Failed to load search index",
        })
      }
    })()

    return () => {
      cancelled = true
    }
  }, [client, searchAssetsState.isLoaded])

  useEffect(() => {
    if (apiSearchState.isLoaded) {
      return
    }

    let cancelled = false

    void (async () => {
      setApiSearchState(current => ({
        ...current,
        isLoading: true,
        error: undefined,
      }))

      try {
        const entries = await loadApiSearchIndex()
        if (cancelled) {
          return
        }

        setApiSearchState({
          entries,
          isLoading: false,
          isLoaded: true,
        })
      } catch (error) {
        if (cancelled) {
          return
        }

        setApiSearchState({
          entries: [],
          isLoading: false,
          isLoaded: true,
          error: error instanceof Error ? error.message : "Failed to load API search index",
        })
      }
    })()

    return () => {
      cancelled = true
    }
  }, [apiSearchState.isLoaded])

  const isSearchIndexLoading =
    (apiSearchState.isLoading || searchAssetsState.isLoading) && searchQuery.trim().length > 0

  return (
    <div
      className={`${styles.searchOverlay} ${isOpen ? styles.searchOverlayOpen : ""}`}
      aria-hidden={!isOpen}
      style={originStyle}
    >
      <button
        type="button"
        className={styles.searchBackdrop}
        aria-label="Close search"
        onClick={onClose}
      />
      <section className={styles.searchPanel} role="dialog" aria-modal="true" aria-label="Search">
        <div className={styles.searchInputRow}>
          <Search size={17} className={styles.searchInputIcon} />
          <input
            ref={searchInputRef}
            className={styles.searchInput}
            value={searchQuery}
            placeholder="Find..."
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
            onChange={event => setSearchQuery(event.target.value)}
            onKeyDown={handleSearchKeyDown}
          />
          <button
            type="button"
            className={styles.searchEscButton}
            aria-label="Close search"
            onClick={onClose}
          >
            <span className={styles.searchEscShortcut}>F</span>
            <span className={styles.searchEscLabel}>Esc</span>
          </button>
        </div>

        <div className={styles.searchResultBody}>
          {searchResults.filter(result => !hiddenNftResultIds.has(result.id)).length === 0 ? (
            <div className={styles.searchEmpty}>
              No matches. Paste an address, a transaction hash, or search by API method, token/NFT
              metadata.
            </div>
          ) : (
            <div className={styles.searchResultList}>
              {searchResults
                .filter(result => !hiddenNftResultIds.has(result.id))
                .map(result => {
                  const Icon = result.icon

                  return (
                    <button
                      key={result.id}
                      type="button"
                      className={styles.searchResultItem}
                      onClick={() => selectSearchResult(result)}
                    >
                      <span className={styles.searchResultIcon}>
                        {result.workspace ? (
                          <span className={styles.searchResultWorkspaceMark} />
                        ) : result.image ? (
                          result.isScam === undefined ? (
                            <img
                              src={result.image}
                              alt=""
                              onError={event => {
                                const fallbackImage = result.fallbackImage
                                if (
                                  fallbackImage &&
                                  !event.currentTarget.src.endsWith(fallbackImage)
                                ) {
                                  event.currentTarget.src = fallbackImage
                                }
                              }}
                            />
                          ) : (
                            <NftImage
                              sources={
                                result.fallbackImage
                                  ? [result.image, result.fallbackImage]
                                  : [result.image]
                              }
                              alt=""
                              blurredClassName={styles.blurredAssetImage}
                              collectionName={result.collectionName}
                              blurred={result.isScam}
                              onNsfw={() => {
                                setHiddenNftResultIds(current => new Set(current).add(result.id))
                              }}
                            />
                          )
                        ) : (
                          <Icon size={17} />
                        )}
                      </span>
                      <span className={styles.searchResultText}>
                        <span className={styles.searchResultTitle}>{result.title}</span>
                        <span className={styles.searchResultDescription}>{result.description}</span>
                      </span>
                    </button>
                  )
                })}
            </div>
          )}

          {isSearchIndexLoading ? (
            <div className={styles.searchIndexState}>Loading search indexes…</div>
          ) : undefined}
          {apiSearchState.error && searchResults.length === 0 ? (
            <div className={styles.searchIndexError}>{apiSearchState.error}</div>
          ) : undefined}
          {searchAssetsState.error && searchResults.length === 0 ? (
            <div className={styles.searchIndexError}>{searchAssetsState.error}</div>
          ) : undefined}
        </div>
      </section>
    </div>
  )
}

function loadSearchAssets(client: TonClient): Promise<LoadedSearchAssets> {
  let cached = searchAssetsCache.get(client)
  if (!cached) {
    cached = Promise.all([
      client.getJettonMasters(undefined, 100, 0),
      client.getNftItems({
        limit: 200,
        offset: 0,
        sortByLastTransactionLt: true,
      }),
    ])
      .then(([tokens, nfts]) => ({tokens, nfts}))
      .catch(error => {
        searchAssetsCache.delete(client)
        throw error
      })
    searchAssetsCache.set(client, cached)
  }
  return cached
}

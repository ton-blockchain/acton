import {
  Boxes,
  ChartNoAxesColumn,
  CircleUserRound,
  Image,
  Search,
  Wallet,
} from "lucide-react"
import type {LucideIcon} from "lucide-react"
import * as React from "react"
import {useNavigate} from "react-router-dom"

import type {TonClient} from "../explorer/api/client"
import type {JettonMaster, NftItem} from "../explorer/api/types"
import {formatAddress, hashToHex, parseAddress} from "../explorer/components/utils"

import {NFT_PLACEHOLDER_IMAGE, TOKEN_PLACEHOLDER_IMAGE} from "./constants"
import {contentString, matchesQuery, shortHash, isTextEntryTarget} from "./dashboardUtils"
import styles from "./DashboardPage.module.css"

interface DashboardSearchProps {
  readonly client: TonClient
}

interface SearchAssetsState {
  readonly tokens: readonly JettonMaster[]
  readonly nfts: readonly NftItem[]
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
}

interface SearchOriginStyle {
  readonly "--search-origin-left"?: string
  readonly "--search-origin-top"?: string
  readonly "--search-origin-width"?: string
  readonly "--search-origin-height"?: string
}

const quickSearchResults: readonly SearchResult[] = [
  {
    id: "quick-home",
    title: "TON Localnet",
    description: "by Acton",
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
  },
  {
    id: "quick-faucet",
    title: "Faucet",
    description: "Send test TON to a wallet",
    href: "/faucet",
    icon: Wallet,
  },
  {
    id: "quick-tokens",
    title: "Tokens",
    description: "Jettons indexed from the local network",
    href: "/tokens",
    icon: Boxes,
  },
  {
    id: "quick-nfts",
    title: "NFTs",
    description: "NFT items indexed from the local network",
    href: "/nfts",
    icon: Image,
  },
]

export const DashboardSearch: React.FC<DashboardSearchProps> = ({client}) => {
  const navigate = useNavigate()
  const [isSearchMounted, setIsSearchMounted] = React.useState(false)
  const [isSearchOpen, setIsSearchOpen] = React.useState(false)
  const [searchQuery, setSearchQuery] = React.useState("")
  const [searchOriginStyle, setSearchOriginStyle] = React.useState<SearchOriginStyle>({})
  const [searchAssetsState, setSearchAssetsState] = React.useState<SearchAssetsState>({
    tokens: [],
    nfts: [],
    isLoading: false,
    isLoaded: false,
  })
  const searchButtonRef = React.useRef<HTMLButtonElement>(null)
  const searchInputRef = React.useRef<HTMLInputElement>(null)
  const searchAnimationRef = React.useRef<number | undefined>(undefined)

  const searchResults = React.useMemo<readonly SearchResult[]>(() => {
    const trimmed = searchQuery.trim()
    if (trimmed.length === 0) {
      return quickSearchResults
    }

    const query = trimmed.toLocaleLowerCase()
    const results: SearchResult[] = []
    const transactionHash = hashToHex(trimmed)
    if (transactionHash) {
      results.push({
        id: `transaction-${transactionHash}`,
        title: "Open transaction",
        description: shortHash(transactionHash),
        href: `/explorer/tx/${encodeURIComponent(transactionHash)}`,
        icon: ChartNoAxesColumn,
      })
    }

    const address = parseAddress(trimmed)?.toString({testOnly: true})
    if (address) {
      results.push({
        id: `address-${address}`,
        title: "Open address",
        description: formatAddress(address, false),
        href: `/explorer/address/${encodeURIComponent(address)}`,
        icon: CircleUserRound,
      })
    }

    for (const token of searchAssetsState.tokens) {
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
        href: `/explorer/address/${encodeURIComponent(token.address)}`,
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
          href: `/explorer/address/${encodeURIComponent(item.address)}`,
          icon: Image,
          image: contentString(item.content, "image"),
          fallbackImage: NFT_PLACEHOLDER_IMAGE,
        })
        if (results.length >= 12) {
          break
        }
      }
    }

    return results
  }, [searchAssetsState.nfts, searchAssetsState.tokens, searchQuery])

  const measureSearchOrigin = React.useCallback(() => {
    const rect = searchButtonRef.current?.getBoundingClientRect()
    if (!rect) {
      return
    }

    setSearchOriginStyle({
      "--search-origin-left": `${rect.left}px`,
      "--search-origin-top": `${rect.top}px`,
      "--search-origin-width": `${rect.width}px`,
      "--search-origin-height": `${rect.height}px`,
    })
  }, [])

  const openSearch = React.useCallback(() => {
    measureSearchOrigin()
    setIsSearchMounted(true)
    if (searchAnimationRef.current !== undefined) {
      cancelAnimationFrame(searchAnimationRef.current)
    }
    searchAnimationRef.current = requestAnimationFrame(() => {
      setIsSearchOpen(true)
    })
  }, [measureSearchOrigin])

  const closeSearch = React.useCallback(() => {
    setIsSearchOpen(false)
  }, [])

  const handleGlobalKeyDown = React.useCallback(
    (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) {
        return
      }

      if (event.key === "Escape" && isSearchMounted) {
        event.preventDefault()
        closeSearch()
        return
      }

      if (event.key.toLocaleLowerCase() !== "f" || isTextEntryTarget(event.target)) {
        return
      }

      event.preventDefault()
      openSearch()
    },
    [closeSearch, isSearchMounted, openSearch],
  )

  const selectSearchResult = React.useCallback(
    (result: SearchResult) => {
      closeSearch()
      setSearchQuery("")
      void navigate(result.href)
    },
    [closeSearch, navigate],
  )

  const handleSearchKeyDown = React.useCallback(
    (event: React.KeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Escape") {
        event.preventDefault()
        closeSearch()
        return
      }
      if (event.key === "Enter" && searchResults[0]) {
        event.preventDefault()
        selectSearchResult(searchResults[0])
      }
    },
    [closeSearch, searchResults, selectSearchResult],
  )

  React.useEffect(() => {
    return () => {
      if (searchAnimationRef.current !== undefined) {
        cancelAnimationFrame(searchAnimationRef.current)
      }
    }
  }, [])

  React.useEffect(() => {
    globalThis.addEventListener("keydown", handleGlobalKeyDown)
    return () => {
      globalThis.removeEventListener("keydown", handleGlobalKeyDown)
    }
  }, [handleGlobalKeyDown])

  React.useEffect(() => {
    if (isSearchMounted && isSearchOpen) {
      searchInputRef.current?.focus()
    }
  }, [isSearchMounted, isSearchOpen])

  React.useEffect(() => {
    if (!isSearchMounted) {
      return
    }

    const handleResize = () => measureSearchOrigin()
    window.addEventListener("resize", handleResize)
    return () => {
      window.removeEventListener("resize", handleResize)
    }
  }, [isSearchMounted, measureSearchOrigin])

  React.useEffect(() => {
    if (!isSearchMounted || isSearchOpen) {
      return
    }

    const timeout = globalThis.setTimeout(() => {
      setIsSearchMounted(false)
    }, 300)

    return () => {
      globalThis.clearTimeout(timeout)
    }
  }, [isSearchMounted, isSearchOpen])

  React.useEffect(() => {
    if (!isSearchMounted || searchAssetsState.isLoaded || searchAssetsState.isLoading) {
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
        const [tokens, nfts] = await Promise.all([
          client.getJettonMasters(undefined, 100, 0),
          client.getNftItems({
            limit: 200,
            offset: 0,
            sortByLastTransactionLt: true,
          }),
        ])

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
  }, [client, isSearchMounted, searchAssetsState.isLoaded, searchAssetsState.isLoading])

  return (
    <>
      <button
        ref={searchButtonRef}
        type="button"
        className={`${styles.searchButton} ${isSearchMounted ? styles.searchButtonMorphing : ""}`}
        onClick={openSearch}
      >
        <span className={styles.searchButtonValue}>
          <Search size={16} />
          <span>Find...</span>
        </span>
        <span className={styles.searchShortcut}>F</span>
      </button>

      {isSearchMounted ? (
        <div
          className={`${styles.searchOverlay} ${isSearchOpen ? styles.searchOverlayOpen : ""}`}
          aria-hidden={!isSearchOpen}
          style={searchOriginStyle as React.CSSProperties}
        >
          <button
            type="button"
            className={styles.searchBackdrop}
            aria-label="Close search"
            onClick={closeSearch}
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
                onClick={closeSearch}
              >
                <span className={styles.searchEscShortcut}>F</span>
                <span className={styles.searchEscLabel}>Esc</span>
              </button>
            </div>

            <div className={styles.searchResultBody}>
              {searchResults.length === 0 ? (
                <div className={styles.searchEmpty}>
                  No matches. Paste an address, a transaction hash, or search by token/NFT metadata.
                </div>
              ) : (
                <div className={styles.searchResultList}>
                  {searchResults.map(result => {
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
                            <img
                              src={result.image}
                              alt=""
                              onError={event => {
                                const fallbackImage = result.fallbackImage
                                if (fallbackImage && !event.currentTarget.src.endsWith(fallbackImage)) {
                                  event.currentTarget.src = fallbackImage
                                }
                              }}
                            />
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

              {searchAssetsState.isLoading && searchQuery.trim().length > 0 ? (
                <div className={styles.searchIndexState}>Loading token and NFT metadata…</div>
              ) : undefined}
              {searchAssetsState.error ? (
                <div className={styles.searchIndexError}>{searchAssetsState.error}</div>
              ) : undefined}
            </div>
          </section>
        </div>
      ) : undefined}
    </>
  )
}

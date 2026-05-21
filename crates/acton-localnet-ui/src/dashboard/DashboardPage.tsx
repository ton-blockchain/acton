import {
  ArrowUpRight,
  BookOpen,
  Boxes,
  ChartNoAxesColumn,
  CircleUserRound,
  Github,
  Image,
  Link2,
  LayoutGrid,
  Moon,
  Search,
  SquareStack,
  Sun,
  Wallet,
} from "lucide-react"
import * as React from "react"
import {Button, Card, CardContent, CardDescription, CardHeader, CardTitle, Input, useToast} from "@acton/shared-ui"
import type {LucideIcon} from "lucide-react"
import {useLocation, useNavigate} from "react-router-dom"

import type {TonClient} from "../explorer/api/client"
import type {JettonMaster, LocalnetNodeInfo, NftItem, V3TransactionListItem} from "../explorer/api/types"

import {formatAddress, formatNano, formatTimeAgo, hashToHex, parseAddress} from "../explorer/components/utils"
import {useAddressName} from "../explorer/hooks/useAddressBook"

import styles from "./DashboardPage.module.css"

interface SidebarItem {
  readonly label: string
  readonly icon: LucideIcon
  readonly path?: string
  readonly href?: string
}

const mainItems: SidebarItem[] = [
  {label: "Home", icon: LayoutGrid, path: "/dashboard"},
  {label: "Explorer", icon: Search, path: "/explorer"},
  {label: "Faucet", icon: Wallet, path: "/dashboard/faucet"},
  {label: "Tokens", icon: Boxes, path: "/dashboard/tokens"},
  {label: "NFTs", icon: Image, path: "/dashboard/nfts"},
]

const footerItems: SidebarItem[] = [
  {label: "Documentation", icon: BookOpen, href: "https://ton-blockchain.github.io/acton/docs/welcome"},
  {label: "GitHub", icon: Github, href: "https://github.com/ton-blockchain/acton"},
]

const quickAmounts = ["1", "5", "20", "100"]
const tokenPlaceholderImage = "/token-placeholder.svg"
const nftPlaceholderImage = tokenPlaceholderImage

interface DashboardPageProps {
  readonly client: TonClient
  readonly theme: string
  readonly setTheme: (theme: string) => void
  readonly children?: React.ReactNode
}

interface HomeState {
  readonly nodeInfo?: LocalnetNodeInfo
  readonly transactions: readonly V3TransactionListItem[]
  readonly accountBalances: Readonly<Record<string, string>>
  readonly isLoading: boolean
  readonly error?: string
}

interface TokensState {
  readonly items: readonly JettonMaster[]
  readonly isLoading: boolean
  readonly error?: string
}

interface NftsState {
  readonly items: readonly NftItem[]
  readonly isLoading: boolean
  readonly error?: string
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

function parseTonAmount(value: string): number | undefined {
  const trimmed = value.trim()
  if (!trimmed || !/^\d+(\.\d{0,9})?$/.test(trimmed)) {
    return undefined
  }

  const [wholePart, fractionPart = ""] = trimmed.split(".")
  const whole = BigInt(wholePart)
  const fraction = BigInt(fractionPart.padEnd(9, "0"))
  const nano = whole * 1_000_000_000n + fraction
  if (nano <= 0n || nano > BigInt(Number.MAX_SAFE_INTEGER)) {
    return undefined
  }
  return Number(nano)
}

function contentString(content: Record<string, unknown> | undefined, key: string): string | undefined {
  const value = content?.[key]
  return typeof value === "string" && value.length > 0 ? value : undefined
}

function formatTokenSupply(token: JettonMaster): string {
  const decimals = Number(token.jetton_content.decimals || 9)
  return (Number(token.total_supply) / 10 ** decimals).toLocaleString()
}

function shortHash(hash: string): string {
  return hash.length > 16 ? `${hash.slice(0, 8)}…${hash.slice(-8)}` : hash
}

function matchesQuery(fields: readonly (string | undefined)[], query: string): boolean {
  return fields.some(field => field?.toLocaleLowerCase().includes(query))
}

function collectRecentAccounts(transactions: readonly V3TransactionListItem[]): string[] {
  const seen = new Set<string>()
  const accounts: string[] = []

  for (const transaction of transactions) {
    if (!seen.has(transaction.account)) {
      seen.add(transaction.account)
      accounts.push(transaction.account)
    }
    if (accounts.length === 6) {
      break
    }
  }

  return accounts
}

const HomeAddressLabel: React.FC<{
  readonly address?: string
  readonly fallback?: string
  readonly className?: string
}> = ({address, fallback = "Unknown", className}) => {
  const normalizedAddress = React.useMemo(() => {
    const parsed = address ? parseAddress(address) : undefined
    return parsed?.toString({testOnly: true})
  }, [address])
  const name = useAddressName(normalizedAddress ?? "")

  if (!normalizedAddress) {
    return <span className={className}>{fallback}</span>
  }

  const shortAddress = formatAddress(normalizedAddress)
  const fullAddress = formatAddress(normalizedAddress, false)

  if (!name) {
    return (
      <span className={`${styles.addressDisplay} ${className ?? ""}`} title={fullAddress}>
        {shortAddress}
      </span>
    )
  }

  return (
    <span className={`${styles.addressDisplay} ${className ?? ""}`} title={`${name} (${fullAddress})`}>
      <span className={styles.addressDisplayName}>{name}</span>
      <span className={styles.addressDisplaySeparator}>·</span>
      <span className={styles.addressDisplayAddress}>{shortAddress}</span>
    </span>
  )
}

export const DashboardPage: React.FC<DashboardPageProps> = ({children, client, theme, setTheme}) => {
  const location = useLocation()
  const navigate = useNavigate()
  const {showToast} = useToast()
  const [address, setAddress] = React.useState("")
  const [amount, setAmount] = React.useState("1")
  const [isSubmitting, setIsSubmitting] = React.useState(false)
  const [isSearchMounted, setIsSearchMounted] = React.useState(false)
  const [isSearchOpen, setIsSearchOpen] = React.useState(false)
  const [searchQuery, setSearchQuery] = React.useState("")
  const searchInputRef = React.useRef<HTMLInputElement>(null)
  const searchAnimationRef = React.useRef<number | undefined>(undefined)
  const [homeState, setHomeState] = React.useState<HomeState>({
    transactions: [],
    accountBalances: {},
    isLoading: true,
  })
  const [tokensState, setTokensState] = React.useState<TokensState>({
    items: [],
    isLoading: false,
  })
  const [nftsState, setNftsState] = React.useState<NftsState>({
    items: [],
    isLoading: false,
  })
  const [searchAssetsState, setSearchAssetsState] = React.useState<SearchAssetsState>({
    tokens: [],
    nfts: [],
    isLoading: false,
    isLoaded: false,
  })

  const amountNano = React.useMemo(() => parseTonAmount(amount), [amount])
  const isFaucetPage = location.pathname === "/dashboard/faucet"
  const isHomePage = location.pathname === "/dashboard"
  const isTokensPage = location.pathname === "/dashboard/tokens"
  const isNftsPage = location.pathname === "/dashboard/nfts"
  const endpoints = React.useMemo(() => client.getEndpoints(), [client])
  const endpointRows = React.useMemo(
    () =>
      [
        {label: "V2 API", value: endpoints.apiV2},
        {label: "V3 API", value: endpoints.apiV3},
      ].filter(endpoint => endpoint.value.length > 0),
    [endpoints],
  )
  const recentAccounts = React.useMemo(() => collectRecentAccounts(homeState.transactions), [homeState.transactions])
  const quickSearchResults = React.useMemo<readonly SearchResult[]>(
    () => [
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
        href: "/dashboard/faucet",
        icon: Wallet,
      },
      {
        id: "quick-tokens",
        title: "Tokens",
        description: "Jettons indexed from the local network",
        href: "/dashboard/tokens",
        icon: Boxes,
      },
      {
        id: "quick-nfts",
        title: "NFTs",
        description: "NFT items indexed from the local network",
        href: "/dashboard/nfts",
        icon: Image,
      },
    ],
    [],
  )
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

    const tokens = searchAssetsState.tokens.length > 0 ? searchAssetsState.tokens : tokensState.items
    const nfts = searchAssetsState.nfts.length > 0 ? searchAssetsState.nfts : nftsState.items

    for (const token of tokens) {
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
        fallbackImage: tokenPlaceholderImage,
      })
      if (results.length >= 12) {
        break
      }
    }

    if (results.length < 12) {
      for (const item of nfts) {
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
          fallbackImage: nftPlaceholderImage,
        })
        if (results.length >= 12) {
          break
        }
      }
    }

    return results
  }, [nftsState.items, quickSearchResults, searchAssetsState.nfts, searchAssetsState.tokens, searchQuery, tokensState.items])

  const openSearch = React.useCallback(() => {
    setIsSearchMounted(true)
    if (searchAnimationRef.current !== undefined) {
      cancelAnimationFrame(searchAnimationRef.current)
    }
    searchAnimationRef.current = requestAnimationFrame(() => {
      setIsSearchOpen(true)
    })
  }, [])

  const closeSearch = React.useCallback(() => {
    setIsSearchOpen(false)
  }, [])

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
    if (isSearchMounted && isSearchOpen) {
      searchInputRef.current?.focus()
    }
  }, [isSearchMounted, isSearchOpen])

  React.useEffect(() => {
    if (!isSearchMounted || isSearchOpen) {
      return
    }

    const timeout = globalThis.setTimeout(() => {
      setIsSearchMounted(false)
    }, 180)

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

  React.useEffect(() => {
    if (!isHomePage) {
      return
    }

    let cancelled = false

    void (async () => {
      setHomeState(current => ({
        ...current,
        isLoading: true,
        error: undefined,
      }))

      try {
        const [nodeInfo, transactionsResponse] = await Promise.all([
          client.getNodeInfo(),
          client.getRecentTransactions(8),
        ])
        const transactions = transactionsResponse.transactions
        const accounts = collectRecentAccounts(transactions)
        let accountBalances: Record<string, string> = {}

        if (accounts.length > 0) {
          try {
            const accountStates = await client.getAccountStates(accounts, false)
            accountBalances = Object.fromEntries(
              accountStates.accounts.map(account => [account.address, account.balance]),
            )
          } catch (error) {
            console.error("Failed to fetch recent account balances", error)
          }
        }

        if (cancelled) {
          return
        }

        setHomeState({
          nodeInfo,
          transactions,
          accountBalances,
          isLoading: false,
        })
      } catch (error) {
        if (cancelled) {
          return
        }

        setHomeState({
          transactions: [],
          accountBalances: {},
          isLoading: false,
          error: error instanceof Error ? error.message : "Failed to load dashboard",
        })
      }
    })()

    return () => {
      cancelled = true
    }
  }, [client, isHomePage])

  React.useEffect(() => {
    if (!isTokensPage) {
      return
    }

    let cancelled = false

    void (async () => {
      setTokensState({
        items: [],
        isLoading: true,
      })

      try {
        const tokens = await client.getJettonMasters(undefined, 100, 0)
        if (cancelled) {
          return
        }
        setTokensState({
          items: tokens,
          isLoading: false,
        })
      } catch (error) {
        if (cancelled) {
          return
        }
        setTokensState({
          items: [],
          isLoading: false,
          error: error instanceof Error ? error.message : "Failed to load tokens",
        })
      }
    })()

    return () => {
      cancelled = true
    }
  }, [client, isTokensPage])

  React.useEffect(() => {
    if (!isNftsPage) {
      return
    }

    let cancelled = false

    void (async () => {
      setNftsState({
        items: [],
        isLoading: true,
      })

      try {
        const nfts = await client.getNftItems({
          limit: 100,
          offset: 0,
          sortByLastTransactionLt: true,
        })
        if (cancelled) {
          return
        }
        setNftsState({
          items: nfts,
          isLoading: false,
        })
      } catch (error) {
        if (cancelled) {
          return
        }
        setNftsState({
          items: [],
          isLoading: false,
          error: error instanceof Error ? error.message : "Failed to load NFTs",
        })
      }
    })()

    return () => {
      cancelled = true
    }
  }, [client, isNftsPage])

  async function handleSubmit(event?: React.FormEvent): Promise<void> {
    event?.preventDefault()
    const trimmedAddress = address.trim()
    const parsedAddress = parseAddress(trimmedAddress)
    if (!parsedAddress) {
      showToast({
        variant: "error",
        title: "Invalid address",
        description: "Enter a valid TON address.",
      })
      return
    }
    if (amountNano === undefined) {
      showToast({
        variant: "error",
        title: "Invalid amount",
        description: "Enter a valid amount greater than zero.",
      })
      return
    }

    const normalized = parsedAddress.toString({testOnly: true})
    setIsSubmitting(true)

    try {
      await client.fundAccount(normalized, amountNano)
      showToast({
        variant: "success",
        title: "Transfer sent",
        description: `Sent ${amount.trim()} TON to ${formatAddress(normalized)}.`,
      })
    } catch (submitError) {
      showToast({
        variant: "error",
        title: "Transfer failed",
        description: submitError instanceof Error ? submitError.message : "Failed to send TON.",
      })
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <div className={styles.page}>
      <aside className={styles.sidebar}>
        <div className={styles.sidebarHeader}>
          <div className={styles.workspaceHeader}>
            <span className={styles.workspaceMark} />
            <span className={styles.workspaceBody}>
              <span className={styles.workspaceName}>TON Localnet</span>
              <span className={styles.workspaceMeta}>by Acton</span>
            </span>
          </div>
        </div>

        <div className={styles.topControls}>
          <button
            type="button"
            className={styles.searchButton}
            onClick={openSearch}
          >
            <div className={styles.searchButtonValue}>
              <Search size={16} />
              <span>Find...</span>
            </div>
            <span className={styles.searchShortcut}>F</span>
          </button>
        </div>

        <div className={styles.navScroll}>
          <nav className={styles.nav}>
            <div className={styles.navSection}>
              {mainItems.map(item => {
                const Icon = item.icon
                const isActive =
                  item.path === "/explorer"
                    ? location.pathname.startsWith("/explorer")
                    : item.path === location.pathname

                return (
                  <button
                    type="button"
                    key={item.label}
                    className={`${styles.navItem} ${isActive ? styles.navItemActive : ""}`}
                    onClick={() => {
                      if (item.path) {
                        void navigate(item.path)
                      }
                    }}
                  >
                    <span className={styles.navItemMain}>
                      <Icon size={18} />
                      <span>{item.label}</span>
                    </span>
                  </button>
                )
              })}
            </div>

            <div className={styles.navDivider} />

            <div className={styles.navFooter}>
              <div className={styles.navSection}>
                {footerItems.map(item => {
                  const Icon = item.icon

                  return (
                    <a
                      key={item.label}
                      className={styles.navItem}
                      href={item.href}
                      target="_blank"
                      rel="noreferrer"
                    >
                      <span className={styles.navItemMain}>
                        <Icon size={18} />
                        <span>{item.label}</span>
                      </span>
                      <ArrowUpRight size={14} />
                    </a>
                  )
                })}
              </div>

              <button
                type="button"
                className={styles.themeSwitch}
                aria-label="Toggle Theme"
                data-theme-toggle=""
                onClick={() => setTheme(theme === "light" ? "dark" : "light")}
              >
                <Sun
                  fill="currentColor"
                  className={`${styles.themeSwitchItem} ${theme === "light" ? styles.themeSwitchItemActive : ""}`}
                />
                <Moon
                  fill="currentColor"
                  className={`${styles.themeSwitchItem} ${theme === "dark" ? styles.themeSwitchItemActive : ""}`}
                />
              </button>
            </div>
          </nav>
        </div>
      </aside>

      {isSearchMounted ? (
        <div
          className={`${styles.searchOverlay} ${isSearchOpen ? styles.searchOverlayOpen : ""}`}
          aria-hidden={!isSearchOpen}
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
              <button type="button" className={styles.searchEscButton} onClick={closeSearch}>
                Esc
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

      <section className={styles.contentArea}>
        <main className={`${styles.content} ${children ? styles.contentEmbedded : ""}`}>
          {children ? <div className={styles.embeddedPage}>{children}</div> : undefined}

          {isHomePage ? (
            <>
              <section className={styles.hero}>
                <div>
                  <h1 className={styles.title}>Home</h1>
                  <p className={styles.subtitle}>A quick snapshot of your local node and recent activity.</p>
                </div>
              </section>

              <section className={styles.homeLayout}>
                <div className={styles.homeMainColumn}>
                  <Card className={`${styles.dashboardCard} ${styles.homeCard}`}>
                    <CardHeader className={styles.dashboardCardHeader}>
                      <div className={styles.cardTitleRow}>
                        <div className={styles.cardIcon}>
                          <ChartNoAxesColumn size={16} />
                        </div>
                        <div>
                          <CardTitle className={styles.dashboardCardTitle}>Recent transactions</CardTitle>
                          <CardDescription className={styles.dashboardCardDescription}>
                            Fresh activity from the local network.
                          </CardDescription>
                        </div>
                      </div>
                    </CardHeader>
                    <CardContent className={styles.dashboardCardContent}>
                      {homeState.error ? (
                        <div className={styles.emptyState}>{homeState.error}</div>
                      ) : homeState.isLoading ? (
                        <div className={styles.emptyState}>Loading transactions…</div>
                      ) : homeState.transactions.length === 0 ? (
                        <div className={styles.emptyState}>No transactions yet.</div>
                      ) : (
                        <div className={styles.listTable}>
                          {homeState.transactions.map(transaction => (
                            <button
                              key={transaction.hash}
                              type="button"
                              className={styles.listRowButton}
                              onClick={() => {
                                const hashHex = hashToHex(transaction.hash)
                                if (hashHex) {
                                  void navigate(`/explorer/tx/${encodeURIComponent(hashHex)}`)
                                }
                              }}
                            >
                              <div className={styles.rowMain}>
                                <div className={styles.rowTopLine}>
                                  <div className={styles.rowPrimary}>
                                    <HomeAddressLabel address={transaction.account} />
                                  </div>
                                  {transaction.description.compute_ph.success ? undefined : (
                                    <span className={`${styles.statusBadge} ${styles.statusError}`}>
                                      Exit {transaction.description.compute_ph.exit_code}
                                    </span>
                                  )}
                                </div>
                                <div className={styles.rowSecondaryDetail}>
                                  From:{" "}
                                  <HomeAddressLabel
                                    address={transaction.in_msg?.source}
                                    className={styles.rowInlineAddress}
                                  />{" "}
                                  · Value: {formatNano(transaction.in_msg?.value || "0")} TON
                                </div>
                              </div>
                              <div className={styles.rowMeta}>
                                <div className={styles.rowPrimary}>#{transaction.mc_block_seqno}</div>
                                <div className={styles.rowSecondary}>{formatTimeAgo(transaction.now)}</div>
                              </div>
                            </button>
                          ))}
                        </div>
                      )}
                    </CardContent>
                  </Card>

                  <Card className={`${styles.dashboardCard} ${styles.homeCard}`}>
                    <CardHeader className={styles.dashboardCardHeader}>
                      <div className={styles.cardTitleRow}>
                        <div className={styles.cardIcon}>
                          <Wallet size={16} />
                        </div>
                        <div>
                          <CardTitle className={styles.dashboardCardTitle}>Recent accounts</CardTitle>
                          <CardDescription className={styles.dashboardCardDescription}>
                            Accounts touched by the latest transactions.
                          </CardDescription>
                        </div>
                      </div>
                    </CardHeader>
                    <CardContent className={styles.dashboardCardContent}>
                      {homeState.error ? (
                        <div className={styles.emptyState}>{homeState.error}</div>
                      ) : homeState.isLoading ? (
                        <div className={styles.emptyState}>Loading accounts…</div>
                      ) : recentAccounts.length === 0 ? (
                        <div className={styles.emptyState}>No accounts yet.</div>
                      ) : (
                        <div className={styles.accountList}>
                          {recentAccounts.map(account => {
                            const balance = homeState.accountBalances[account]

                            return (
                              <button
                                key={account}
                                type="button"
                                className={styles.accountItem}
                                onClick={() => {
                                  void navigate(`/explorer/address/${encodeURIComponent(account)}`)
                                }}
                              >
                                <span className={styles.accountIcon}>
                                  <CircleUserRound size={14} />
                                </span>
                                <span className={styles.accountText}>
                                  <span className={styles.accountName}>
                                    <HomeAddressLabel address={account} />
                                  </span>
                                  <span className={styles.accountBalance}>
                                    {balance === undefined ? "Balance unavailable" : `${formatNano(balance)} TON`}
                                  </span>
                                </span>
                                <ArrowUpRight size={14} className={styles.accountArrow} />
                              </button>
                            )
                          })}
                        </div>
                      )}
                    </CardContent>
                  </Card>
                </div>

                <aside className={styles.homeSideColumn}>
                  <Card className={`${styles.dashboardCard} ${styles.homeCard}`}>
                    <CardHeader className={styles.dashboardCardHeader}>
                      <div className={styles.cardTitleRow}>
                        <div className={styles.cardIcon}>
                          <SquareStack size={16} />
                        </div>
                        <div>
                          <CardTitle className={styles.dashboardCardTitle}>Current block</CardTitle>
                          <CardDescription className={styles.dashboardCardDescription}>
                            Latest masterchain sequence number.
                          </CardDescription>
                        </div>
                      </div>
                    </CardHeader>
                    <CardContent className={styles.dashboardCardContent}>
                      <div className={styles.metricValue}>
                        {homeState.nodeInfo ? `#${homeState.nodeInfo.last_block_seqno}` : "—"}
                      </div>
                      <div className={styles.metricMeta}>
                        {homeState.nodeInfo ? `${homeState.nodeInfo.uptime_seconds}s uptime` : "Waiting for node info"}
                      </div>
                    </CardContent>
                  </Card>

                  <Card className={`${styles.dashboardCard} ${styles.homeCard}`}>
                    <CardHeader className={styles.dashboardCardHeader}>
                      <div className={styles.cardTitleRow}>
                        <div className={styles.cardIcon}>
                          <Link2 size={16} />
                        </div>
                        <div>
                          <CardTitle className={styles.dashboardCardTitle}>Endpoints</CardTitle>
                          <CardDescription className={styles.dashboardCardDescription}>
                            Active local URLs for the current node.
                          </CardDescription>
                        </div>
                      </div>
                    </CardHeader>
                    <CardContent className={`${styles.dashboardCardContent} ${styles.endpointList}`}>
                      {endpointRows.map(endpoint => (
                        <div key={endpoint.label} className={styles.endpointRow}>
                          <span className={styles.endpointLabel}>{endpoint.label}</span>
                          <code className={styles.endpointValue}>{endpoint.value}</code>
                        </div>
                      ))}
                    </CardContent>
                  </Card>
                </aside>
              </section>
            </>
          ) : undefined}

          {isTokensPage ? (
            <>
              <section className={styles.hero}>
                <div>
                  <h1 className={styles.title}>Tokens</h1>
                  <p className={styles.subtitle}>Jettons detected on the local network.</p>
                </div>
              </section>

              <section className={styles.resourceGrid}>
                {tokensState.error ? (
                  <div className={styles.emptyState}>{tokensState.error}</div>
                ) : tokensState.isLoading ? (
                  <div className={styles.emptyState}>Loading tokens…</div>
                ) : tokensState.items.length === 0 ? (
                  <div className={styles.emptyState}>No tokens yet.</div>
                ) : (
                  tokensState.items.map(token => {
                    const symbol = token.jetton_content.symbol || "???"
                    const name = token.jetton_content.name || "Unknown Jetton"
                    const image = token.jetton_content.image || tokenPlaceholderImage

                    return (
                      <Card
                        key={token.address}
                        className={`${styles.dashboardCard} ${styles.assetCard}`}
                        role="button"
                        tabIndex={0}
                        onClick={() => {
                          void navigate(`/explorer/address/${encodeURIComponent(token.address)}`)
                        }}
                        onKeyDown={event => {
                          if (event.key === "Enter" || event.key === " ") {
                            event.preventDefault()
                            void navigate(`/explorer/address/${encodeURIComponent(token.address)}`)
                          }
                        }}
                      >
                        <CardHeader className={styles.dashboardCardHeader}>
                          <div className={styles.cardTitleRow}>
                            <img
                              src={image}
                              alt=""
                              className={styles.assetImage}
                              onError={event => {
                                const imageElement = event.currentTarget
                                if (imageElement.getAttribute("src") !== tokenPlaceholderImage) {
                                  imageElement.src = tokenPlaceholderImage
                                }
                              }}
                            />
                            <div>
                              <CardTitle className={styles.dashboardCardTitle}>{name}</CardTitle>
                              <CardDescription className={styles.dashboardCardDescription}>
                                {symbol}
                              </CardDescription>
                            </div>
                          </div>
                        </CardHeader>
                        <CardContent className={styles.dashboardCardContent}>
                          <div className={styles.assetMetaGrid}>
                            <div>
                              <span className={styles.assetMetaLabel}>Supply</span>
                              <span className={styles.assetMetaValue}>{formatTokenSupply(token)}</span>
                            </div>
                            <div>
                              <span className={styles.assetMetaLabel}>Mintable</span>
                              <span className={styles.assetMetaValue}>{token.mintable ? "Yes" : "No"}</span>
                            </div>
                          </div>
                        </CardContent>
                      </Card>
                    )
                  })
                )}
              </section>
            </>
          ) : undefined}

          {isNftsPage ? (
            <>
              <section className={styles.hero}>
                <div>
                  <h1 className={styles.title}>NFTs</h1>
                  <p className={styles.subtitle}>NFT items indexed from the local network.</p>
                </div>
              </section>

              <section className={styles.resourceGrid}>
                {nftsState.error ? (
                  <div className={styles.emptyState}>{nftsState.error}</div>
                ) : nftsState.isLoading ? (
                  <div className={styles.emptyState}>Loading NFTs…</div>
                ) : nftsState.items.length === 0 ? (
                  <div className={styles.emptyState}>No NFTs yet.</div>
                ) : (
                  nftsState.items.map(item => {
                    const name = contentString(item.content, "name") || "NFT Item"
                    const image = contentString(item.content, "image") || nftPlaceholderImage
                    const collectionName = contentString(item.collection?.collection_content, "name") || "Standalone"

                    return (
                      <Card
                        key={item.address}
                        className={`${styles.dashboardCard} ${styles.assetCard}`}
                        role="button"
                        tabIndex={0}
                        onClick={() => {
                          void navigate(`/explorer/address/${encodeURIComponent(item.address)}`)
                        }}
                        onKeyDown={event => {
                          if (event.key === "Enter" || event.key === " ") {
                            event.preventDefault()
                            void navigate(`/explorer/address/${encodeURIComponent(item.address)}`)
                          }
                        }}
                      >
                        <CardHeader className={styles.dashboardCardHeader}>
                          <div className={styles.cardTitleRow}>
                            <img
                              src={image}
                              alt=""
                              className={styles.assetImage}
                              onError={event => {
                                const imageElement = event.currentTarget
                                if (imageElement.getAttribute("src") !== nftPlaceholderImage) {
                                  imageElement.src = nftPlaceholderImage
                                }
                              }}
                            />
                            <div>
                              <CardTitle className={styles.dashboardCardTitle}>{name}</CardTitle>
                              <CardDescription className={styles.dashboardCardDescription}>
                                #{item.index}
                              </CardDescription>
                            </div>
                          </div>
                        </CardHeader>
                        <CardContent className={styles.dashboardCardContent}>
                          <div className={styles.assetMetaGrid}>
                            <div>
                              <span className={styles.assetMetaLabel}>Collection</span>
                              <span className={styles.assetMetaValue}>{collectionName}</span>
                            </div>
                            <div>
                              <span className={styles.assetMetaLabel}>Sale</span>
                              <span className={styles.assetMetaValue}>{item.on_sale ? "Listed" : "Not listed"}</span>
                            </div>
                          </div>
                        </CardContent>
                      </Card>
                    )
                  })
                )}
              </section>
            </>
          ) : undefined}

          {isFaucetPage ? (
            <>
              <section className={styles.hero}>
                <div>
                  <h1 className={styles.title}>Send test TON</h1>
                  <p className={styles.subtitle}>Top up any wallet address with test TON in a few seconds.</p>
                </div>
              </section>

              <section className={styles.faucetLayout}>
                <form className={styles.formCard} onSubmit={event => void handleSubmit(event)}>
                  <div className={styles.cardHeader}>
                    <div className={styles.cardTitleRow}>
                      <div className={styles.cardIcon}>
                        <Wallet size={16} />
                      </div>
                      <div>
                        <h2 className={styles.cardTitle}>Wallet top up</h2>
                        <p className={styles.cardDescription}>Enter an address, choose an amount, and send funds.</p>
                      </div>
                    </div>
                  </div>

                  <div className={styles.fieldBlock}>
                    <label className={styles.label} htmlFor="dashboard-address">
                      Recipient address
                    </label>
                    <Input
                      id="dashboard-address"
                      className={styles.fieldInput}
                      placeholder="EQ..."
                      value={address}
                      onChange={event => setAddress(event.target.value)}
                    />
                    <p className={styles.hint}>Paste any raw or user-friendly TON address.</p>
                  </div>

                  <div className={styles.fieldBlock}>
                    <label className={styles.label} htmlFor="dashboard-amount">
                      Amount
                    </label>
                    <div className={styles.amountRow}>
                  <Input
                    id="dashboard-amount"
                    className={styles.fieldInput}
                    inputMode="decimal"
                    placeholder="0.0 TON"
                    value={amount}
                    onChange={event => setAmount(event.target.value)}
                  />
                    </div>
                  </div>
                    <div className={styles.quickActions}>
                      {quickAmounts.map(value => (
                        <Button
                          key={value}
                          type="button"
                          variant={amount === value ? "secondary" : "outline"}
                          size="sm"
                          className={styles.quickActionButton}
                          onClick={() => setAmount(value)}
                        >
                          {value} TON
                        </Button>
                      ))}
                    </div>

                  <div className={styles.formFooter}>
                    <div className={styles.formHint}>Use this faucet to fund wallets for testing.</div>
                    <Button
                      type="submit"
                      className={styles.sendButton}
                      disabled={isSubmitting || address.trim().length === 0 || amount.trim().length === 0}
                    >
                      <span>{isSubmitting ? "Sending..." : "Send TON"}</span>
                      <ArrowUpRight size={16} />
                    </Button>
                  </div>
                </form>
              </section>
            </>
          ) : undefined}
        </main>
      </section>
    </div>
  )
}

import {
  Card,
  CardContent,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@acton/shared-ui"
import {
  ArrowDownLeft,
  ArrowUpRight,
  Braces,
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  Coins,
  Filter,
  History,
  Image,
  MoreHorizontal,
  UsersRound,
} from "lucide-react"
import type React from "react"
import {lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState} from "react"
import {useNavigate} from "react-router-dom"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import type {
  FullAccountState,
  JettonMaster,
  JettonWallet,
  NftItem,
  Message,
  Transaction,
  VerificationSourceResponse,
} from "../api/types"
import type {TonClient} from "../api/client"
import {addressKey, buildMessageNamesByOpcodeHex} from "../api/compilerAbi"

import {AddressLabel} from "./AddressLabel"
import {Nfts} from "./Nfts"
import {Tokens} from "./Tokens"
import styles from "./AccountDetails.module.css"
import {formatNano, formatTimeAgo, hashToHex, isSameAddress, parseAddress} from "./utils"

type Tabs = "history" | "contract" | "tokens" | "nfts" | "holders"
const ContractCode = lazy(async () => {
  const module = await import("./ContractCode")
  return {default: module.ContractCode}
})

interface AccountDetailsProps {
  readonly transactions: Transaction[]
  readonly accountState?: FullAccountState
  readonly compilerAbi?: ContractABI
  readonly compilerAbiLoading?: boolean
  readonly compilerAbiError?: string
  readonly verifiedSource?: VerificationSourceResponse
  readonly verifiedSourceLoading?: boolean
  readonly ownerAddress: string
  readonly jettonWallets: JettonWallet[]
  readonly nftItems: NftItem[]
  readonly jettonMaster?: JettonMaster
  readonly holders?: JettonWallet[]
  readonly tokensLoading?: boolean
  readonly nftsLoading?: boolean
  readonly holdersLoading?: boolean
  readonly transactionsLoading?: boolean
  readonly transactionsError?: string
  readonly accountLoading?: boolean
  readonly showHoldersTab?: boolean
  readonly client: TonClient
  readonly onAddressClick?: (addr: string) => void
  readonly activeTabHash?: string
  readonly onTabChange?: (tab: Tabs) => void
}

const ITEMS_PER_PAGE = 10
const TRANSACTION_FILTERS_STORAGE_KEY = "acton.account.transactionFilters.v1"
type PaginationItem = number | "ellipsis-left" | "ellipsis-right"
type AccountSortOrder = "desc" | "asc"
type AccountTimeFormat = "relative" | "smart" | "absolute"

interface AccountTransactionFilters {
  readonly hiddenActionKeys: readonly string[]
  readonly sortOrder: AccountSortOrder
  readonly timeFormat: AccountTimeFormat
}

interface HistoryTransactionInfo {
  readonly isIncoming: boolean
  readonly address: string
  readonly displayAddressFallback: string
  readonly displayMessage?: Message
  readonly actionKey: string
  readonly actionLabel: string
  readonly displayValue: bigint
}

interface HistoryTransactionRow {
  readonly tx: Transaction
  readonly info: HistoryTransactionInfo
}

type MessageNamesByAddress = Map<
  string,
  {incoming: Map<string, string>; outgoing: Map<string, string>}
>

interface FilterPopoverPosition {
  readonly top: number
  readonly left: number
}

const DEFAULT_TRANSACTION_FILTERS: AccountTransactionFilters = {
  hiddenActionKeys: [],
  sortOrder: "desc",
  timeFormat: "smart",
}

const TIME_FORMAT_OPTIONS: readonly {
  readonly value: AccountTimeFormat
  readonly label: string
  readonly preview: string
}[] = [
  {value: "relative", label: "Relative", preview: "20 hours ago"},
  {value: "smart", label: "Smart", preview: "Combined"},
  {value: "absolute", label: "Absolute", preview: "25 May, 09:41"},
]

export const AccountDetails: React.FC<AccountDetailsProps> = ({
  transactions,
  accountState,
  compilerAbi,
  compilerAbiLoading = false,
  compilerAbiError,
  verifiedSource,
  verifiedSourceLoading = false,
  ownerAddress,
  jettonWallets,
  nftItems,
  jettonMaster,
  holders,
  tokensLoading = false,
  nftsLoading = false,
  holdersLoading = false,
  transactionsLoading = false,
  transactionsError,
  accountLoading = false,
  showHoldersTab = false,
  client,
  onAddressClick,
  activeTabHash,
  onTabChange,
}) => {
  const navigate = useNavigate()
  const [activeTab, setActiveTab] = useState<Tabs>("history")
  const filterPopoverRef = useRef<HTMLDivElement>(null)
  const filterButtonRef = useRef<HTMLButtonElement>(null)
  const [compilerAbiByAddress, setCompilerAbiByAddress] = useState<
    Map<string, ContractABI | undefined>
  >(new Map())
  const [isFiltersOpen, setIsFiltersOpen] = useState(false)
  const [filterPopoverPosition, setFilterPopoverPosition] = useState<
    FilterPopoverPosition | undefined
  >()
  const [transactionFilters, setTransactionFilters] =
    useState<AccountTransactionFilters>(readTransactionFilters)

  useEffect(() => {
    if (
      activeTabHash &&
      (activeTabHash === "history" ||
        activeTabHash === "contract" ||
        activeTabHash === "tokens" ||
        activeTabHash === "nfts" ||
        activeTabHash === "holders")
    ) {
      setActiveTab(activeTabHash as Tabs)
    }
  }, [activeTabHash])

  useEffect(() => {
    let isActive = true

    const loadRelatedCompilerAbis = async () => {
      if (activeTab !== "history") {
        return
      }

      const addresses = new Set<string>()
      addresses.add(ownerAddress)

      for (const tx of transactions) {
        if (tx.in_msg.source) addresses.add(tx.in_msg.source)
        if (tx.in_msg.destination) addresses.add(tx.in_msg.destination)
        for (const msg of tx.out_msgs) {
          if (msg.source) addresses.add(msg.source)
          if (msg.destination) addresses.add(msg.destination)
        }
      }

      const requestedAddresses = [...addresses].filter(Boolean)
      if (requestedAddresses.length === 0) {
        setCompilerAbiByAddress(new Map())
        return
      }

      const next = new Map<string, ContractABI | undefined>()
      const ownerKey = addressKey(ownerAddress)
      const stateRequestAddresses = requestedAddresses.filter(
        address => addressKey(address) !== ownerKey,
      )

      const states =
        stateRequestAddresses.length > 0
          ? await client.getAccountStates(stateRequestAddresses, false).catch(() => {})
          : undefined
      const addressToCodeHash = new Map<string, string>()
      if (compilerAbi) {
        next.set(ownerKey, compilerAbi)
      }
      for (const account of states?.accounts ?? []) {
        if (account.code_hash) {
          addressToCodeHash.set(addressKey(account.address), account.code_hash)
        }
      }

      const codeHashesToFetch = new Set<string>()
      for (const codeHash of addressToCodeHash.values()) {
        codeHashesToFetch.add(codeHash)
      }

      const codeHashes = [...codeHashesToFetch]
      const fetchedAbis =
        codeHashes.length > 0
          ? await client
              .getCompilerAbis(codeHashes)
              .catch((): Awaited<ReturnType<TonClient["getCompilerAbis"]>> => ({}))
          : {}
      const abiByCodeHash = new Map<string, ContractABI | undefined>()
      for (const codeHash of codeHashes) {
        abiByCodeHash.set(codeHash, fetchedAbis[codeHash]?.compiler_abi)
      }

      for (const address of requestedAddresses) {
        const key = addressKey(address)
        if (key === ownerKey) {
          next.set(key, compilerAbi)
          continue
        }
        const codeHash = addressToCodeHash.get(key)
        next.set(
          key,
          codeHash ? (abiByCodeHash.get(codeHash) ?? undefined) : (next.get(key) ?? undefined),
        )
      }

      if (!isActive) return
      setCompilerAbiByAddress(next)
    }

    void loadRelatedCompilerAbis()
    return () => {
      isActive = false
    }
  }, [transactions, ownerAddress, compilerAbi, activeTab, client])

  const handleTabClick = (tab: Tabs) => {
    setActiveTab(tab)
    onTabChange?.(tab)
  }

  const [currentPage, setCurrentPage] = useState(1)
  const [hoveredAddress, setHoveredAddress] = useState<string | undefined>()
  const [nowSeconds, setNowSeconds] = useState(() => Math.floor(Date.now() / 1000))

  useEffect(() => {
    if (activeTab !== "history" || transactions.length === 0) return

    const updateNow = () => setNowSeconds(Math.floor(Date.now() / 1000))
    updateNow()

    const interval = globalThis.setInterval(updateNow, 5000)
    return () => globalThis.clearInterval(interval)
  }, [activeTab, transactions.length])

  const browsedAddr = useMemo(() => parseAddress(ownerAddress), [ownerAddress])
  const messageNamesByAddress = useMemo(() => {
    const next = new Map<string, {incoming: Map<string, string>; outgoing: Map<string, string>}>()
    for (const [address, abi] of compilerAbiByAddress) {
      next.set(address, {
        incoming: buildMessageNamesByOpcodeHex(abi, "incoming_messages"),
        outgoing: buildMessageNamesByOpcodeHex(abi, "outgoing_messages"),
      })
    }
    return next
  }, [compilerAbiByAddress])
  const transactionRows = useMemo<readonly HistoryTransactionRow[]>(
    () =>
      transactions.map(tx => ({
        tx,
        info: getHistoryTransactionInfo(tx, browsedAddr, messageNamesByAddress),
      })),
    [transactions, browsedAddr, messageNamesByAddress],
  )
  const actionFilterOptions = useMemo(() => {
    const options = new Map<string, {key: string; label: string; count: number}>()
    for (const row of transactionRows) {
      const existing = options.get(row.info.actionKey)
      options.set(row.info.actionKey, {
        key: row.info.actionKey,
        label: row.info.actionLabel,
        count: (existing?.count ?? 0) + 1,
      })
    }
    return [...options.values()]
  }, [transactionRows])
  const hiddenActionKeys = useMemo(
    () => new Set(transactionFilters.hiddenActionKeys),
    [transactionFilters.hiddenActionKeys],
  )
  const visibleTransactionRows = useMemo(() => {
    const next = transactionRows
      .filter(row => !hiddenActionKeys.has(row.info.actionKey))
      .sort((left, right) => {
        const comparison = compareTransactionsByTime(left.tx, right.tx)
        return transactionFilters.sortOrder === "desc" ? -comparison : comparison
      })
    return next
  }, [transactionRows, hiddenActionKeys, transactionFilters.sortOrder])
  const totalPages = Math.max(1, Math.ceil(visibleTransactionRows.length / ITEMS_PER_PAGE))
  const safeCurrentPage = Math.min(currentPage, totalPages)
  const startIndex = (safeCurrentPage - 1) * ITEMS_PER_PAGE
  const paginatedTransactionRows = visibleTransactionRows.slice(
    startIndex,
    startIndex + ITEMS_PER_PAGE,
  )
  const paginationItems = useMemo(
    () => getPaginationItems(safeCurrentPage, totalPages),
    [safeCurrentPage, totalPages],
  )

  useEffect(() => {
    try {
      globalThis.localStorage?.setItem(
        TRANSACTION_FILTERS_STORAGE_KEY,
        JSON.stringify(transactionFilters),
      )
    } catch {
      // Ignore storage errors in private browsing or restricted environments.
    }
  }, [transactionFilters])

  useEffect(() => {
    if (!isFiltersOpen) return

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target
      if (target instanceof Node && filterPopoverRef.current?.contains(target)) {
        return
      }
      setIsFiltersOpen(false)
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsFiltersOpen(false)
      }
    }

    document.addEventListener("pointerdown", handlePointerDown, true)
    document.addEventListener("keydown", handleKeyDown)
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown, true)
      document.removeEventListener("keydown", handleKeyDown)
    }
  }, [isFiltersOpen])

  useEffect(() => {
    setCurrentPage(1)
  }, [ownerAddress, transactionFilters.hiddenActionKeys, transactionFilters.sortOrder])

  useEffect(() => {
    setCurrentPage(page => Math.min(page, totalPages))
  }, [totalPages])

  const setSortOrder = (sortOrder: AccountSortOrder) => {
    setTransactionFilters(filters => ({...filters, sortOrder}))
  }

  const setTimeFormat = (timeFormat: AccountTimeFormat) => {
    setTransactionFilters(filters => ({...filters, timeFormat}))
  }

  const toggleActionFilter = (actionKey: string) => {
    setTransactionFilters(filters => {
      const hidden = new Set(filters.hiddenActionKeys)
      if (hidden.has(actionKey)) {
        hidden.delete(actionKey)
      } else {
        hidden.add(actionKey)
      }
      return {...filters, hiddenActionKeys: [...hidden]}
    })
  }

  const updateFilterPopoverPosition = useCallback(() => {
    const button = filterButtonRef.current
    if (!button) {
      return
    }

    const rect = button.getBoundingClientRect()
    const viewportWidth = globalThis.innerWidth || document.documentElement.clientWidth
    const sidePadding = viewportWidth <= 520 ? 12 : 16
    const width = Math.max(240, Math.min(430, viewportWidth - sidePadding * 2))
    const left = Math.min(
      Math.max(sidePadding, rect.right - width),
      Math.max(sidePadding, viewportWidth - width - sidePadding),
    )

    setFilterPopoverPosition({
      top: rect.bottom + 6,
      left,
    })
  }, [])

  useEffect(() => {
    if (!isFiltersOpen) return

    updateFilterPopoverPosition()
    globalThis.addEventListener("resize", updateFilterPopoverPosition)
    document.addEventListener("scroll", updateFilterPopoverPosition, true)
    return () => {
      globalThis.removeEventListener("resize", updateFilterPopoverPosition)
      document.removeEventListener("scroll", updateFilterPopoverPosition, true)
    }
  }, [isFiltersOpen, updateFilterPopoverPosition])

  const filtersPopoverStyle = filterPopoverPosition
    ? ({
        top: `${filterPopoverPosition.top}px`,
        left: `${filterPopoverPosition.left}px`,
      } as React.CSSProperties)
    : undefined

  return (
    <Card className={styles.tableCard}>
      <div className={styles.tabs}>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "history" ? styles.tabActive : ""}`}
          onClick={() => handleTabClick("history")}
        >
          <span className={styles.tabIcon} aria-hidden="true">
            <History size={18} />
          </span>
          History
        </button>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "tokens" ? styles.tabActive : ""}`}
          onClick={() => handleTabClick("tokens")}
        >
          <span className={styles.tabIcon} aria-hidden="true">
            <Coins size={18} />
          </span>
          Tokens
        </button>
        {(showHoldersTab || jettonMaster) && (
          <button
            type="button"
            className={`${styles.tab} ${activeTab === "holders" ? styles.tabActive : ""}`}
            onClick={() => handleTabClick("holders")}
          >
            <span className={styles.tabIcon} aria-hidden="true">
              <UsersRound size={18} />
            </span>
            Holders
          </button>
        )}
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "nfts" ? styles.tabActive : ""}`}
          onClick={() => handleTabClick("nfts")}
        >
          <span className={styles.tabIcon} aria-hidden="true">
            <Image size={18} />
          </span>
          Collectibles
        </button>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "contract" ? styles.tabActive : ""}`}
          onClick={() => handleTabClick("contract")}
        >
          <span className={styles.tabIcon} aria-hidden="true">
            <Braces size={18} />
          </span>
          Contract
        </button>
        <div className={styles.flexSpacer} />
        {activeTab === "history" && (
          <>
            <div className={styles.tab}>
              <span className={styles.tabIcon} aria-hidden="true">
                <CalendarDays size={17} />
              </span>
            </div>
            <div className={styles.filterPopoverRoot} ref={filterPopoverRef}>
              <button
                ref={filterButtonRef}
                type="button"
                className={styles.tab}
                onClick={() => {
                  updateFilterPopoverPosition()
                  setIsFiltersOpen(open => !open)
                }}
                aria-haspopup="dialog"
                aria-expanded={isFiltersOpen}
              >
                <span className={styles.tabIcon} aria-hidden="true">
                  <Filter size={17} />
                </span>
                Filters
              </button>
              {isFiltersOpen && (
                <div
                  className={styles.filtersPopover}
                  style={filtersPopoverStyle}
                  role="dialog"
                  aria-label="History filters"
                >
                  <section className={styles.filterSection}>
                    <div className={styles.filterSectionTitle}>Actions</div>
                    <div className={styles.actionFiltersList}>
                      {actionFilterOptions.length === 0 ? (
                        <div className={styles.filterEmptyState}>No actions yet.</div>
                      ) : (
                        actionFilterOptions.map(option => {
                          const selected = !hiddenActionKeys.has(option.key)
                          return (
                            <button
                              key={option.key}
                              type="button"
                              className={`${styles.actionFilterChip} ${
                                selected ? styles.actionFilterChipSelected : ""
                              }`}
                              onClick={() => toggleActionFilter(option.key)}
                              aria-pressed={selected}
                              title={`${option.label} (${option.count})`}
                            >
                              {option.label}
                            </button>
                          )
                        })
                      )}
                    </div>
                  </section>

                  <section className={styles.filterSection}>
                    <div className={styles.filterSectionTitle}>Sort</div>
                    <div className={styles.segmentedControl}>
                      <button
                        type="button"
                        className={`${styles.segmentedOption} ${
                          transactionFilters.sortOrder === "desc"
                            ? styles.segmentedOptionActive
                            : ""
                        }`}
                        onClick={() => setSortOrder("desc")}
                      >
                        Newest first
                      </button>
                      <button
                        type="button"
                        className={`${styles.segmentedOption} ${
                          transactionFilters.sortOrder === "asc" ? styles.segmentedOptionActive : ""
                        }`}
                        onClick={() => setSortOrder("asc")}
                      >
                        Oldest first
                      </button>
                    </div>
                  </section>

                  <section className={styles.filterSection}>
                    <div className={styles.filterSectionTitle}>Time format</div>
                    <div className={styles.timeFormatGrid}>
                      {TIME_FORMAT_OPTIONS.map(option => (
                        <button
                          key={option.value}
                          type="button"
                          className={`${styles.timeFormatOption} ${
                            transactionFilters.timeFormat === option.value
                              ? styles.timeFormatOptionActive
                              : ""
                          }`}
                          onClick={() => setTimeFormat(option.value)}
                          aria-pressed={transactionFilters.timeFormat === option.value}
                        >
                          <span className={styles.timeFormatLabel}>{option.label}</span>
                          <span className={styles.timeFormatPreview}>{option.preview}</span>
                        </button>
                      ))}
                    </div>
                  </section>
                </div>
              )}
            </div>
          </>
        )}
      </div>

      {activeTab === "history" ? (
        <CardContent className={styles.historyContent}>
          <Table>
            <TableHeader className={styles.historyHeaderGroup}>
              <TableRow className={styles.historyHeaderRow}>
                <TableHead className={`${styles.tableHeader} ${styles.timeColumn}`}>Time</TableHead>
                <TableHead className={`${styles.tableHeader} ${styles.actionColumn}`}>
                  Action
                </TableHead>
                <TableHead className={styles.tableHeader}>Address</TableHead>
                <TableHead className={`${styles.tableHeader} ${styles.valueContainer}`}>
                  Value
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {transactionsLoading ? (
                Array.from({length: ITEMS_PER_PAGE}, (_, index) => (
                  <TableRow key={`transaction-skeleton-${index}`} className={styles.skeletonRow}>
                    <TableCell className={`${styles.time} ${styles.timeColumn}`}>
                      <div className={`${styles.skeleton} ${styles.historySkeletonTime}`} />
                    </TableCell>
                    <TableCell className={styles.actionColumn}>
                      <div className={styles.action}>
                        <div className={`${styles.skeleton} ${styles.historySkeletonIcon}`} />
                        <div className={`${styles.skeleton} ${styles.historySkeletonAction}`} />
                      </div>
                    </TableCell>
                    <TableCell>
                      <div className={`${styles.skeleton} ${styles.historySkeletonAddress}`} />
                    </TableCell>
                    <TableCell className={styles.valueContainer}>
                      <div className={`${styles.skeleton} ${styles.historySkeletonValue}`} />
                    </TableCell>
                  </TableRow>
                ))
              ) : transactionsError ? (
                <TableRow className={styles.emptyRow}>
                  <TableCell colSpan={4} className={styles.emptyCell}>
                    <div className={`${styles.tableState} ${styles.tableStateError}`}>
                      Failed to load transactions: {transactionsError}
                    </div>
                  </TableCell>
                </TableRow>
              ) : paginatedTransactionRows.length === 0 ? (
                <TableRow className={styles.emptyRow}>
                  <TableCell colSpan={4} className={styles.emptyCell}>
                    <div className={styles.tableState}>
                      {transactions.length > 0
                        ? "No transactions match filters."
                        : "No transactions found."}
                    </div>
                  </TableCell>
                </TableRow>
              ) : (
                paginatedTransactionRows.map(({tx, info}) => {
                  const valueStr = formatNano(info.displayValue.toString())
                  const formattedTime = formatTransactionTime(
                    tx.utime,
                    nowSeconds,
                    transactionFilters.timeFormat,
                  )
                  const isAddressHovered =
                    hoveredAddress && info.address
                      ? isSameAddress(info.address, hoveredAddress)
                      : false

                  return (
                    <TableRow
                      key={tx.hash}
                      className={`${styles.row} ${styles.clickableRow}`}
                      onClick={() => {
                        const txHash = hashToHex(tx.hash)
                        if (!txHash) return
                        void navigate(`/explorer/tx/${txHash}`)
                      }}
                    >
                      <TableCell className={`${styles.time} ${styles.timeColumn}`}>
                        <span title={formattedTime.title}>{formattedTime.label}</span>
                      </TableCell>
                      <TableCell className={styles.actionColumn}>
                        <div className={styles.action}>
                          {info.isIncoming ? (
                            <ArrowDownLeft
                              className={`${styles.actionIcon} ${styles.statusSuccess}`}
                            />
                          ) : (
                            <ArrowUpRight
                              className={`${styles.actionIcon} ${styles.statusFailed}`}
                            />
                          )}
                          {info.actionLabel ? (
                            <span className={`${styles.actionText} ${styles.opcode}`}>
                              {info.actionLabel}
                            </span>
                          ) : (
                            <span className={styles.actionText}>Transaction</span>
                          )}
                        </div>
                      </TableCell>
                      <TableCell>
                        <div className={styles.addressWrapper}>
                          <button
                            type="button"
                            className={`${styles.address} ${isAddressHovered ? styles.addressHighlighted : ""}`}
                            onClick={e => {
                              e.stopPropagation()
                              if (info.address) onAddressClick?.(info.address)
                            }}
                            onMouseEnter={() => info.address && setHoveredAddress(info.address)}
                            onMouseLeave={() => setHoveredAddress(undefined)}
                          >
                            <AddressLabel
                              address={info.address}
                              fallback={info.displayAddressFallback}
                            />
                          </button>
                        </div>
                      </TableCell>
                      <TableCell className={styles.valueContainer}>
                        <div
                          className={`${info.isIncoming ? styles.valuePositive : styles.valueNegative} ${styles.historyValue}`}
                        >
                          {info.isIncoming ? "+" : "-"}{" "}
                          {Number.parseFloat(valueStr).toLocaleString()} GRAM
                        </div>
                      </TableCell>
                    </TableRow>
                  )
                })
              )}
            </TableBody>
          </Table>

          {transactionsLoading ? (
            <div className={styles.pagination}>
              <div className={styles.paginationControls} aria-hidden="true">
                <div
                  className={`${styles.paginationButton} ${styles.paginationSkeletonButton} ${styles.skeleton}`}
                />
                {Array.from({length: 5}, (_, index) => (
                  <div
                    key={`pagination-skeleton-${index}`}
                    className={`${styles.paginationPage} ${styles.paginationSkeletonPage} ${styles.skeleton}`}
                  />
                ))}
                <div
                  className={`${styles.paginationButton} ${styles.paginationSkeletonButton} ${styles.skeleton}`}
                />
              </div>
            </div>
          ) : (
            !transactionsError &&
            totalPages > 1 && (
              <div className={styles.pagination}>
                <div className={styles.paginationControls}>
                  <button
                    type="button"
                    className={styles.paginationButton}
                    onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                    disabled={safeCurrentPage === 1}
                    aria-label="Previous page"
                  >
                    <ChevronLeft size={16} />
                    Previous
                  </button>
                  {paginationItems.map(item =>
                    typeof item === "number" ? (
                      <button
                        key={item}
                        type="button"
                        className={`${styles.paginationPage} ${
                          item === safeCurrentPage ? styles.paginationPageActive : ""
                        }`}
                        onClick={() => setCurrentPage(item)}
                        aria-current={item === safeCurrentPage ? "page" : undefined}
                      >
                        {item}
                      </button>
                    ) : (
                      <span key={item} className={styles.paginationEllipsis} aria-hidden="true">
                        <MoreHorizontal size={16} />
                      </span>
                    ),
                  )}
                  <button
                    type="button"
                    className={styles.paginationButton}
                    onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
                    disabled={safeCurrentPage === totalPages}
                    aria-label="Next page"
                  >
                    Next
                    <ChevronRight size={16} />
                  </button>
                </div>
              </div>
            )
          )}
        </CardContent>
      ) : activeTab === "tokens" ? (
        <CardContent className={styles.tokensContent}>
          {tokensLoading ? (
            <div className={styles.emptyState}>Loading tokens...</div>
          ) : (
            <Tokens wallets={jettonWallets} client={client} onAddressClick={onAddressClick} />
          )}
        </CardContent>
      ) : activeTab === "nfts" ? (
        <CardContent className={styles.tokensContent}>
          {nftsLoading ? (
            <div className={styles.emptyState}>Loading NFTs...</div>
          ) : (
            <Nfts items={nftItems} onAddressClick={onAddressClick} />
          )}
        </CardContent>
      ) : activeTab === "holders" ? (
        <CardContent className={styles.historyContent}>
          {holdersLoading ? (
            <div className={styles.emptyState}>Loading holders...</div>
          ) : (
            <Table>
              <TableHeader className={styles.historyHeaderGroup}>
                <TableRow className={styles.historyHeaderRow}>
                  <TableHead className={styles.tableHeader}>Owner</TableHead>
                  <TableHead className={styles.tableHeader}>Wallet</TableHead>
                  <TableHead className={`${styles.tableHeader} ${styles.valueContainer}`}>
                    Balance
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {(holders || []).map(holder => {
                  const decimals = Number(jettonMaster?.jetton_content?.decimals || 9)
                  const balance = Number(holder.balance) / 10 ** decimals
                  const symbol = jettonMaster?.jetton_content?.symbol || ""

                  return (
                    <TableRow
                      key={holder.address}
                      className={`${styles.row} ${styles.clickableRow}`}
                      onClick={() => onAddressClick?.(holder.owner)}
                    >
                      <TableCell>
                        <button
                          type="button"
                          className={styles.address}
                          onClick={e => {
                            e.stopPropagation()
                            onAddressClick?.(holder.owner)
                          }}
                        >
                          <AddressLabel address={holder.owner} />
                        </button>
                      </TableCell>
                      <TableCell>
                        <button
                          type="button"
                          className={styles.address}
                          onClick={e => {
                            e.stopPropagation()
                            onAddressClick?.(holder.address)
                          }}
                        >
                          <AddressLabel address={holder.address} />
                        </button>
                      </TableCell>
                      <TableCell className={styles.valueContainer}>
                        <div className={styles.valuePositive}>
                          {balance.toLocaleString(undefined, {maximumFractionDigits: decimals})}{" "}
                          {symbol}
                        </div>
                      </TableCell>
                    </TableRow>
                  )
                })}
                {(!holders || holders.length === 0) && (
                  <TableRow className={styles.emptyRow}>
                    <TableCell colSpan={3} className={styles.emptyCell}>
                      <div className={styles.emptyState}>No holders found.</div>
                    </TableCell>
                  </TableRow>
                )}
              </TableBody>
            </Table>
          )}
        </CardContent>
      ) : (
        <CardContent className={styles.tokensContent}>
          {accountLoading && !accountState ? (
            <div className={styles.contractSkeleton}>
              <div className={`${styles.skeleton} ${styles.contractSkeletonTabs}`} />
              <div className={`${styles.skeleton} ${styles.contractSkeletonBlock}`} />
            </div>
          ) : (
            <Suspense fallback={<div className={styles.emptyState}>Loading contract code...</div>}>
              <ContractCode
                codeBoc={accountState?.code ?? ""}
                dataBoc={accountState?.data}
                compilerAbi={compilerAbi}
                compilerAbiLoading={compilerAbiLoading}
                compilerAbiError={compilerAbiError}
                verifiedSource={verifiedSource}
                verifiedSourceLoading={verifiedSourceLoading}
                onContractClick={onAddressClick}
              />
            </Suspense>
          )}
        </CardContent>
      )}
    </Card>
  )
}

function getPaginationItems(currentPage: number, totalPages: number): PaginationItem[] {
  if (totalPages <= 7) {
    return Array.from({length: totalPages}, (_, index) => index + 1)
  }

  if (currentPage <= 4) {
    return [1, 2, 3, 4, 5, "ellipsis-right", totalPages]
  }

  if (currentPage >= totalPages - 3) {
    return [
      1,
      "ellipsis-left",
      totalPages - 4,
      totalPages - 3,
      totalPages - 2,
      totalPages - 1,
      totalPages,
    ]
  }

  return [
    1,
    "ellipsis-left",
    currentPage - 1,
    currentPage,
    currentPage + 1,
    "ellipsis-right",
    totalPages,
  ]
}

function readTransactionFilters(): AccountTransactionFilters {
  try {
    const raw = globalThis.localStorage?.getItem(TRANSACTION_FILTERS_STORAGE_KEY)
    if (!raw) {
      return DEFAULT_TRANSACTION_FILTERS
    }

    const parsed: unknown = JSON.parse(raw)
    if (!isRecord(parsed)) {
      return DEFAULT_TRANSACTION_FILTERS
    }

    const hiddenActionKeys = Array.isArray(parsed.hiddenActionKeys)
      ? parsed.hiddenActionKeys.filter((value): value is string => typeof value === "string")
      : []
    const sortOrder: AccountSortOrder = parsed.sortOrder === "asc" ? "asc" : "desc"
    const timeFormat: AccountTimeFormat =
      parsed.timeFormat === "relative" ||
      parsed.timeFormat === "smart" ||
      parsed.timeFormat === "absolute"
        ? parsed.timeFormat
        : "smart"

    return {hiddenActionKeys, sortOrder, timeFormat}
  } catch {
    return DEFAULT_TRANSACTION_FILTERS
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null
}

function getHistoryTransactionInfo(
  tx: Transaction,
  browsedAddr: ReturnType<typeof parseAddress>,
  messageNamesByAddress: MessageNamesByAddress,
): HistoryTransactionInfo {
  const inMsg = tx.in_msg
  const inMsgSrc = parseAddress(inMsg.source || "")
  const inMsgDest = parseAddress(inMsg.destination || "")
  const isInboundToAccount = inMsgDest && browsedAddr ? inMsgDest.equals(browsedAddr) : false
  const isIncoming =
    isInboundToAccount &&
    browsedAddr !== undefined &&
    inMsgSrc !== undefined &&
    (!inMsgSrc.equals(browsedAddr) || tx.out_msgs.length === 0)

  const inValue = BigInt(tx.in_msg.value || "0")
  const outValue = tx.out_msgs.reduce((acc, msg) => acc + BigInt(msg.value || "0"), BigInt(0))
  const displayValue = isIncoming ? inValue : outValue
  const address = isIncoming
    ? tx.in_msg.source || ""
    : tx.out_msgs.find(message => message.destination)?.destination || ""
  const displayAddressFallback = isIncoming ? "External" : "Contract"
  const displayMessage = isIncoming
    ? tx.in_msg
    : tx.out_msgs.find(message => message.destination) ||
      tx.out_msgs.find(message => message.opcode) ||
      tx.out_msgs[0]
  const opcode = displayMessage?.opcode?.trim()
  const normalizedOpcode = opcode ? normalizeOpcode(opcode) : undefined
  const actionLabel =
    resolveMessageName(displayMessage, messageNamesByAddress) ||
    opcode ||
    (isIncoming ? "Received GRAM" : "Sent GRAM")
  const actionKey = normalizedOpcode
    ? `opcode:${isIncoming ? "in" : "out"}:${normalizedOpcode}`
    : `direction:${isIncoming ? "incoming" : "outgoing"}`

  return {
    isIncoming,
    address,
    displayAddressFallback,
    displayMessage,
    actionKey,
    actionLabel,
    displayValue,
  }
}

function compareTransactionsByTime(left: Transaction, right: Transaction): number {
  if (left.utime !== right.utime) {
    return left.utime - right.utime
  }

  const ltComparison = compareBigIntStrings(left.transaction_id.lt, right.transaction_id.lt)
  if (ltComparison !== 0) {
    return ltComparison
  }

  return left.hash.localeCompare(right.hash)
}

function compareBigIntStrings(left: string, right: string): number {
  try {
    const leftValue = BigInt(left)
    const rightValue = BigInt(right)
    if (leftValue < rightValue) return -1
    if (leftValue > rightValue) return 1
    return 0
  } catch {
    return left.localeCompare(right)
  }
}

function formatTransactionTime(
  utime: number,
  nowSeconds: number,
  timeFormat: AccountTimeFormat,
): {label: string; title: string} {
  const absolute = formatAbsoluteTime(utime)
  if (timeFormat === "absolute") {
    return {label: absolute, title: absolute}
  }

  if (timeFormat === "relative") {
    return {label: formatRelativeTime(utime, nowSeconds), title: absolute}
  }

  return {label: formatTimeAgo(utime, nowSeconds), title: absolute}
}

function formatRelativeTime(utime: number, nowSeconds: number): string {
  const diff = Math.max(0, nowSeconds - utime)

  if (diff === 0) return "right now"
  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86_400) return `${Math.floor(diff / 3600)}h ago`
  if (diff < 604_800) return `${Math.floor(diff / 86_400)}d ago`
  if (diff < 2_629_800) return `${Math.floor(diff / 604_800)}w ago`
  if (diff < 31_557_600) return `${Math.floor(diff / 2_629_800)}mo ago`
  return `${Math.floor(diff / 31_557_600)}y ago`
}

function formatAbsoluteTime(utime: number): string {
  const date = new Date(utime * 1000)
  const day = date.toLocaleString("default", {day: "numeric"})
  const month = date.toLocaleString("default", {month: "short"})
  const time = date.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  })
  return `${day} ${month}, ${time}`
}

function resolveMessageName(
  message: Message | undefined,
  messageNamesByAddress: MessageNamesByAddress,
): string | undefined {
  if (!message?.opcode) {
    return undefined
  }

  const opcode = normalizeOpcode(message.opcode)
  if (!opcode) {
    return undefined
  }

  const destinationNames = message.destination
    ? messageNamesByAddress.get(addressKey(message.destination))
    : undefined
  const sourceNames = message.source
    ? messageNamesByAddress.get(addressKey(message.source))
    : undefined

  return destinationNames?.incoming.get(opcode) ?? sourceNames?.outgoing.get(opcode) ?? undefined
}

function normalizeOpcode(opcode: string): string | undefined {
  const normalized = opcode.trim()
  if (!normalized) {
    return undefined
  }

  try {
    const value =
      normalized.startsWith("0x") || normalized.startsWith("0X")
        ? Number.parseInt(normalized.slice(2), 16)
        : Number.parseInt(normalized, 10)

    if (!Number.isInteger(value) || value < 0 || value > 0xff_ff_ff_ff) {
      return undefined
    }

    return `0x${value.toString(16).padStart(8, "0")}`
  } catch {
    return undefined
  }
}

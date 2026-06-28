import {Suspense, lazy, useCallback, useEffect, useMemo, useRef, useState} from "react"
import type {CSSProperties, FC, JSX, MouseEvent} from "react"
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
  BadgeDollarSign,
  BadgeMinus,
  BadgePlus,
  Bell,
  Braces,
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  CircleDot,
  CircleX,
  Code2,
  Coins,
  Database,
  FileCode2,
  Filter,
  Flame,
  Gavel,
  Globe2,
  History,
  Image,
  ImageIcon,
  ImagePlus,
  KeyRound,
  Landmark,
  Layers,
  LockKeyhole,
  MoreHorizontal,
  MoveDownLeft,
  MoveUpRight,
  Network,
  PackagePlus,
  Pickaxe,
  RefreshCw,
  ServerCog,
  ShieldCheck,
  SquareStack,
  UsersRound,
  Vault,
  WalletCards,
  Webhook,
  type LucideIcon,
} from "lucide-react"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import type {
  AddressInformation,
  AccountStateTokenInfo,
  JettonMaster,
  JettonWallet,
  NftItem,
  V3Action,
  V3Metadata,
  V3Message,
  V3TransactionListItem,
  VerificationSourceResponse,
} from "../api/types"
import type {TonClient} from "../api/client"
import {addressKey} from "../api/compilerAbi"
import {
  collectTransactionListAddresses,
  useMessageNamesByAddress,
  type MessageNamesByAddress,
} from "../hooks/useMessageNamesByAddress"

import {AddressChip} from "./AddressChip"
import {AddressLabel} from "./AddressLabel"
import {Nfts} from "./Nfts"
import {Tokens, TokensSkeleton} from "./Tokens"
import styles from "./AccountDetails.module.css"
import {formatNano, formatTimeAgo, hashToHex, isSameAddress, parseAddress} from "./utils"

type Tabs = "history" | "contract" | "tokens" | "nfts" | "holders"

interface AccountDetailsProps {
  readonly transactions: V3TransactionListItem[]
  readonly actions?: V3Action[]
  readonly actionMetadata?: V3Metadata
  readonly highlightedTransactionHashes?: readonly string[]
  readonly accountState?: AddressInformation
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
  readonly transactionsHasMore?: boolean
  readonly transactionsLoadingMore?: boolean
  readonly transactionsPaginated?: boolean
  readonly actionsSupported?: boolean
  readonly actionsLoading?: boolean
  readonly actionsError?: string
  readonly actionsHasMore?: boolean
  readonly actionsLoadingMore?: boolean
  readonly accountLoading?: boolean
  readonly showHoldersTab?: boolean
  readonly client: TonClient
  readonly onAddressClick?: (addr: string, event?: MouseEvent<HTMLElement>) => void
  readonly onTransactionClick?: (hash: string, event?: MouseEvent<HTMLElement>) => void
  readonly onLoadMoreTransactions?: () => void
  readonly onLoadMoreActions?: () => void
  readonly activeTabHash?: string
  readonly onTabChange?: (tab: Tabs) => void
}

const ITEMS_PER_PAGE = 10
const TRANSACTION_SKELETON_ROWS = 5
const TRANSACTION_FILTERS_STORAGE_KEY = "acton.account.transactionFilters.v1"
type PaginationItem = number | "ellipsis-left" | "ellipsis-right"
type AccountHistoryMode = "actions" | "transactions"
type AccountSortOrder = "desc" | "asc"
export type AccountTimeFormat = "relative" | "smart" | "absolute"
type HistoryValueTone = "positive" | "negative" | "empty" | "neutral"

interface HistoryTechnicalLabel {
  readonly label: string
}

interface AccountTransactionFilters {
  readonly historyMode: AccountHistoryMode
  readonly hiddenActionKeys: readonly string[]
  readonly hiddenToncenterActionKeys: readonly string[]
  readonly sortOrder: AccountSortOrder
  readonly timeFormat: AccountTimeFormat
}

interface HistoryTransactionInfo {
  readonly isIncoming: boolean
  readonly address: string
  readonly displayAddressFallback: string
  readonly displayMessage?: V3Message
  readonly actionKey: string
  readonly actionLabel: string
  readonly technicalLabel?: HistoryTechnicalLabel
  readonly displayValue: bigint
}

interface HistoryTransactionRow {
  readonly tx: V3TransactionListItem
  readonly info: HistoryTransactionInfo
}

interface HistoryTextValueLine {
  readonly kind: "text"
  readonly label: string
  readonly tone: HistoryValueTone
}

interface HistorySwapValueLine {
  readonly kind: "swap"
  readonly from: HistoryTextValueLine
  readonly to: HistoryTextValueLine
}

type HistoryValueLine = HistoryTextValueLine | HistorySwapValueLine

export interface HistoryActionInfo {
  readonly rowKey: string
  readonly transactionHash?: string
  readonly transactionHashes: readonly string[]
  readonly utime: number
  readonly isIncoming: boolean
  readonly success: boolean
  readonly address: string
  readonly displayAddressFallback: string
  readonly relationLabel?: string
  readonly actionKey: string
  readonly actionLabel: string
  readonly technicalLabel?: HistoryTechnicalLabel
  readonly valueLines: readonly HistoryValueLine[]
}

export interface HistoryActionRow {
  readonly action: V3Action
  readonly info: HistoryActionInfo
}

interface FilterPopoverPosition {
  readonly top: number
  readonly left: number
}

const DEFAULT_TRANSACTION_FILTERS: AccountTransactionFilters = {
  historyMode: "actions",
  hiddenActionKeys: [],
  hiddenToncenterActionKeys: [],
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

export const AccountDetails: FC<AccountDetailsProps> = ({
  transactions,
  actions = [],
  actionMetadata = {},
  highlightedTransactionHashes = [],
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
  transactionsHasMore = false,
  transactionsLoadingMore = false,
  transactionsPaginated = false,
  actionsSupported = false,
  actionsLoading = false,
  actionsError,
  actionsHasMore = false,
  actionsLoadingMore = false,
  accountLoading = false,
  showHoldersTab = false,
  client,
  onAddressClick,
  onTransactionClick,
  onLoadMoreTransactions,
  onLoadMoreActions,
  activeTabHash,
  onTabChange,
}) => {
  const [activeTab, setActiveTab] = useState<Tabs>("history")
  const filterPopoverRef = useRef<HTMLDivElement>(null)
  const filterButtonRef = useRef<HTMLButtonElement>(null)
  const [isFiltersOpen, setIsFiltersOpen] = useState(false)
  const [filterPopoverPosition, setFilterPopoverPosition] = useState<
    FilterPopoverPosition | undefined
  >()
  const [transactionFilters, setTransactionFilters] =
    useState<AccountTransactionFilters>(readTransactionFilters)
  const showNftsTab = !nftsLoading && nftItems.length > 0
  const effectiveHistoryMode: AccountHistoryMode =
    actionsSupported && transactionFilters.historyMode === "actions" ? "actions" : "transactions"
  const activeHistorySourceCount =
    effectiveHistoryMode === "actions" ? actions.length : transactions.length

  useEffect(() => {
    if (
      activeTabHash &&
      (activeTabHash === "history" ||
        activeTabHash === "contract" ||
        activeTabHash === "tokens" ||
        (activeTabHash === "nfts" && showNftsTab) ||
        activeTabHash === "holders")
    ) {
      setActiveTab(activeTabHash as Tabs)
    }
  }, [activeTabHash, showNftsTab])

  useEffect(() => {
    if (activeTab !== "nfts" || showNftsTab) {
      return
    }

    setActiveTab("history")
    onTabChange?.("history")
  }, [activeTab, onTabChange, showNftsTab])

  const handleTabClick = (tab: Tabs) => {
    setActiveTab(tab)
    onTabChange?.(tab)
  }

  const [currentPage, setCurrentPage] = useState(1)
  const [hoveredAddress, setHoveredAddress] = useState<string | undefined>()
  const [nowSeconds, setNowSeconds] = useState(() => Math.floor(Date.now() / 1000))

  useEffect(() => {
    if (activeTab !== "history" || activeHistorySourceCount === 0) return

    const updateNow = () => setNowSeconds(Math.floor(Date.now() / 1000))
    updateNow()

    const interval = globalThis.setInterval(updateNow, 5000)
    return () => globalThis.clearInterval(interval)
  }, [activeTab, activeHistorySourceCount])

  const browsedAddr = useMemo(() => parseAddress(ownerAddress), [ownerAddress])
  const transactionAddresses = useMemo(
    () => collectTransactionListAddresses(transactions),
    [transactions],
  )
  const actionAddresses = useMemo(() => collectActionMessageNameAddresses(actions), [actions])
  const messageNameAddresses = useMemo(
    () => [...transactionAddresses, ...actionAddresses],
    [transactionAddresses, actionAddresses],
  )
  const messageNamesByAddress = useMessageNamesByAddress({
    client,
    addresses: messageNameAddresses,
  })
  const transactionRows = useMemo<readonly HistoryTransactionRow[]>(
    () =>
      transactions.map(tx => ({
        tx,
        info: getHistoryTransactionInfo(tx, browsedAddr, messageNamesByAddress),
      })),
    [transactions, browsedAddr, messageNamesByAddress],
  )
  const actionRows = useMemo<readonly HistoryActionRow[]>(
    () => buildHistoryActionRows(actions, ownerAddress, actionMetadata, messageNamesByAddress),
    [actions, ownerAddress, actionMetadata, messageNamesByAddress],
  )
  const highlightedTransactionHashSet = useMemo(
    () => new Set(highlightedTransactionHashes),
    [highlightedTransactionHashes],
  )
  const actionFilterOptions = useMemo(() => {
    const options = new Map<string, {key: string; label: string; count: number}>()
    const rows = effectiveHistoryMode === "actions" ? actionRows : transactionRows
    for (const row of rows) {
      const existing = options.get(row.info.actionKey)
      options.set(row.info.actionKey, {
        key: row.info.actionKey,
        label: row.info.actionLabel,
        count: (existing?.count ?? 0) + 1,
      })
    }
    return [...options.values()]
  }, [actionRows, effectiveHistoryMode, transactionRows])
  const hiddenActionKeys = useMemo(
    () =>
      new Set(
        effectiveHistoryMode === "actions"
          ? transactionFilters.hiddenToncenterActionKeys
          : transactionFilters.hiddenActionKeys,
      ),
    [
      effectiveHistoryMode,
      transactionFilters.hiddenActionKeys,
      transactionFilters.hiddenToncenterActionKeys,
    ],
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
  const visibleActionRows = useMemo(() => {
    const next = actionRows
      .filter(row => !hiddenActionKeys.has(row.info.actionKey))
      .sort((left, right) => {
        const comparison = compareActionsByTime(left.info, right.info)
        return transactionFilters.sortOrder === "desc" ? -comparison : comparison
      })
    return next
  }, [actionRows, hiddenActionKeys, transactionFilters.sortOrder])
  const currentHistoryPaginated = effectiveHistoryMode === "transactions" && transactionsPaginated
  const totalPages = currentHistoryPaginated
    ? Math.max(1, Math.ceil(visibleTransactionRows.length / ITEMS_PER_PAGE))
    : 1
  const safeCurrentPage = Math.min(currentPage, totalPages)
  const startIndex = (safeCurrentPage - 1) * ITEMS_PER_PAGE
  const displayedTransactionRows = currentHistoryPaginated
    ? visibleTransactionRows.slice(startIndex, startIndex + ITEMS_PER_PAGE)
    : visibleTransactionRows
  const displayedActionRows = visibleActionRows
  const displayedHistoryRowsLength =
    effectiveHistoryMode === "actions"
      ? displayedActionRows.length
      : displayedTransactionRows.length
  const activeHistoryLoading =
    effectiveHistoryMode === "actions" ? actionsLoading : transactionsLoading
  const activeHistoryError = effectiveHistoryMode === "actions" ? actionsError : transactionsError
  const activeHistoryHasMore =
    effectiveHistoryMode === "actions" ? actionsHasMore : transactionsHasMore
  const activeHistoryLoadingMore =
    effectiveHistoryMode === "actions" ? actionsLoadingMore : transactionsLoadingMore
  const activeLoadMoreHistory =
    effectiveHistoryMode === "actions" ? onLoadMoreActions : onLoadMoreTransactions
  const historySubject = effectiveHistoryMode === "actions" ? "actions" : "transactions"
  const paginationItems = useMemo(
    () => getPaginationItems(safeCurrentPage, totalPages),
    [safeCurrentPage, totalPages],
  )
  const showLoadMoreHistory =
    !activeHistoryLoading &&
    !activeHistoryError &&
    activeHistoryHasMore &&
    activeLoadMoreHistory !== undefined

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
  }, [
    ownerAddress,
    effectiveHistoryMode,
    transactionFilters.hiddenActionKeys,
    transactionFilters.hiddenToncenterActionKeys,
    transactionFilters.sortOrder,
  ])

  useEffect(() => {
    setCurrentPage(page => Math.min(page, totalPages))
  }, [totalPages])

  const setSortOrder = (sortOrder: AccountSortOrder) => {
    setTransactionFilters(filters => ({...filters, sortOrder}))
  }

  const setTimeFormat = (timeFormat: AccountTimeFormat) => {
    setTransactionFilters(filters => ({...filters, timeFormat}))
  }

  const setHistoryMode = (historyMode: AccountHistoryMode) => {
    setTransactionFilters(filters => ({...filters, historyMode}))
  }

  const toggleActionFilter = (actionKey: string) => {
    setTransactionFilters(filters => {
      const hidden = new Set(
        effectiveHistoryMode === "actions"
          ? filters.hiddenToncenterActionKeys
          : filters.hiddenActionKeys,
      )
      if (hidden.has(actionKey)) {
        hidden.delete(actionKey)
      } else {
        hidden.add(actionKey)
      }
      return effectiveHistoryMode === "actions"
        ? {...filters, hiddenToncenterActionKeys: [...hidden]}
        : {...filters, hiddenActionKeys: [...hidden]}
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
      } as CSSProperties)
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
        {showNftsTab && (
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
        )}
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
                  {actionsSupported && (
                    <section className={styles.filterSection}>
                      <div className={styles.filterSectionTitle}>History View</div>
                      <div className={styles.segmentedControl}>
                        <button
                          type="button"
                          className={`${styles.segmentedOption} ${
                            effectiveHistoryMode === "actions" ? styles.segmentedOptionActive : ""
                          }`}
                          onClick={() => setHistoryMode("actions")}
                        >
                          Actions
                        </button>
                        <button
                          type="button"
                          className={`${styles.segmentedOption} ${
                            effectiveHistoryMode === "transactions"
                              ? styles.segmentedOptionActive
                              : ""
                          }`}
                          onClick={() => setHistoryMode("transactions")}
                        >
                          Transactions
                        </button>
                      </div>
                    </section>
                  )}

                  <section className={styles.filterSection}>
                    <div className={styles.filterSectionTitle}>Actions</div>
                    <div className={styles.actionFiltersList}>
                      {actionFilterOptions.length === 0 ? (
                        <div className={styles.filterEmptyState}>No actions yet</div>
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
                <TableHead className={`${styles.tableHeader} ${styles.technicalColumn}`} />
                <TableHead className={`${styles.tableHeader} ${styles.valueContainer}`}>
                  Value
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {activeHistoryLoading ? (
                Array.from({length: TRANSACTION_SKELETON_ROWS}, (_, index) => (
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
                    <TableCell className={styles.technicalColumn}>
                      <div className={`${styles.skeleton} ${styles.historySkeletonTechnical}`} />
                    </TableCell>
                    <TableCell className={styles.valueContainer}>
                      <div className={`${styles.skeleton} ${styles.historySkeletonValue}`} />
                    </TableCell>
                  </TableRow>
                ))
              ) : activeHistoryError ? (
                <TableRow className={styles.emptyRow}>
                  <TableCell colSpan={5} className={styles.emptyCell}>
                    <div className={`${styles.tableState} ${styles.tableStateError}`}>
                      Failed to load {historySubject}: {activeHistoryError}
                    </div>
                  </TableCell>
                </TableRow>
              ) : displayedHistoryRowsLength === 0 ? (
                <TableRow className={styles.emptyRow}>
                  <TableCell colSpan={5} className={styles.emptyCell}>
                    <div className={styles.tableState}>
                      {activeHistorySourceCount > 0
                        ? `No ${historySubject} match filters`
                        : `No ${historySubject} found`}
                    </div>
                  </TableCell>
                </TableRow>
              ) : effectiveHistoryMode === "actions" ? (
                <ActionHistoryRows
                  rows={displayedActionRows}
                  nowSeconds={nowSeconds}
                  timeFormat={transactionFilters.timeFormat}
                  highlightedTransactionHashSet={highlightedTransactionHashSet}
                  onAddressClick={onAddressClick}
                  onTransactionClick={onTransactionClick}
                />
              ) : (
                displayedTransactionRows.map(({tx, info}) => {
                  const transactionHash = tx.hash
                  const valueStr = formatNano(info.displayValue.toString())
                  const isEmptyValue = info.displayValue === 0n
                  const valuePrefix = isEmptyValue ? "" : info.isIncoming ? "+ " : "- "
                  const valueLabel = isEmptyValue
                    ? "empty"
                    : `${valuePrefix}${Number.parseFloat(valueStr).toLocaleString()} GRAM`
                  const formattedTime = formatTransactionTime(
                    tx.now,
                    nowSeconds,
                    transactionFilters.timeFormat,
                  )
                  const isAddressHovered =
                    hoveredAddress && info.address
                      ? isSameAddress(info.address, hoveredAddress)
                      : false
                  const isHighlighted = highlightedTransactionHashSet.has(transactionHash)

                  return (
                    <TableRow
                      key={transactionHash ?? `${tx.account}:${tx.lt}:${tx.now}`}
                      className={`${styles.row} ${styles.clickableRow} ${
                        isHighlighted ? styles.newTransactionRow : ""
                      }`}
                      onClick={event => {
                        const txHash = hashToHex(transactionHash)
                        if (!txHash) return
                        onTransactionClick?.(txHash, event)
                      }}
                    >
                      <TableCell className={`${styles.time} ${styles.timeColumn}`}>
                        <span title={formattedTime.title}>{formattedTime.label}</span>
                      </TableCell>
                      <TableCell className={styles.actionColumn}>
                        <div className={styles.action}>
                          {info.isIncoming ? (
                            <MoveDownLeft
                              className={`${styles.actionIcon} ${styles.statusSuccess}`}
                              aria-hidden="true"
                            />
                          ) : (
                            <MoveUpRight
                              className={`${styles.actionIcon} ${styles.statusFailed}`}
                              aria-hidden="true"
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
                          {info.address ? (
                            <AddressChip
                              address={info.address}
                              fallback={info.displayAddressFallback}
                              highlighted={isAddressHovered}
                              onAddressClick={onAddressClick}
                              onHoverAddressChange={setHoveredAddress}
                            />
                          ) : (
                            <span className={styles.addressFallback}>
                              {info.displayAddressFallback}
                            </span>
                          )}
                        </div>
                      </TableCell>
                      <TableCell className={styles.technicalColumn}>
                        <HistoryTechnicalCell technicalLabel={info.technicalLabel} />
                      </TableCell>
                      <TableCell className={styles.valueContainer}>
                        <div
                          className={`${
                            isEmptyValue
                              ? styles.valueEmpty
                              : info.isIncoming
                                ? styles.valuePositive
                                : styles.valueNegative
                          } ${styles.historyValue}`}
                        >
                          {valueLabel}
                        </div>
                      </TableCell>
                    </TableRow>
                  )
                })
              )}
            </TableBody>
          </Table>

          {activeHistoryLoading ? (
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
            !activeHistoryError &&
            currentHistoryPaginated &&
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
          {showLoadMoreHistory && activeLoadMoreHistory && (
            <div className={styles.pagination}>
              <div className={styles.paginationControls}>
                <button
                  type="button"
                  className={styles.paginationButton}
                  onClick={activeLoadMoreHistory}
                  disabled={activeHistoryLoadingMore}
                >
                  {activeHistoryLoadingMore ? "Loading..." : "Load more"}
                </button>
              </div>
            </div>
          )}
        </CardContent>
      ) : activeTab === "tokens" ? (
        <CardContent className={styles.tokensContent}>
          {tokensLoading ? (
            <TokensSkeleton />
          ) : (
            <Tokens wallets={jettonWallets} client={client} onAddressClick={onAddressClick} />
          )}
        </CardContent>
      ) : activeTab === "nfts" && showNftsTab ? (
        <CardContent className={styles.tokensContent}>
          <Nfts items={nftItems} onAddressClick={onAddressClick} />
        </CardContent>
      ) : activeTab === "holders" ? (
        <CardContent className={styles.historyContent}>
          {holdersLoading ? (
            <HoldersSkeleton />
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
                      onClick={event => onAddressClick?.(holder.owner, event)}
                    >
                      <TableCell>
                        <button
                          type="button"
                          className={styles.address}
                          onClick={e => {
                            e.stopPropagation()
                            onAddressClick?.(holder.owner, e)
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
                            onAddressClick?.(holder.address, e)
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
                      <div className={styles.emptyState}>No holders found</div>
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
            <ContractCodeSkeleton />
          ) : (
            <Suspense fallback={<ContractCodeSkeleton />}>
              <ContractCode
                codeBoc={accountState?.code ?? ""}
                ownerAddress={ownerAddress}
                client={client}
                dataBoc={accountState?.data ?? undefined}
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

function HoldersSkeleton(): JSX.Element {
  return (
    <Table aria-label="Loading holders">
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
        {Array.from({length: 4}, (_, index) => (
          <TableRow key={`holders-skeleton-${index}`} className={styles.skeletonRow}>
            <TableCell>
              <div className={`${styles.skeleton} ${styles.historySkeletonAddress}`} />
            </TableCell>
            <TableCell>
              <div className={`${styles.skeleton} ${styles.historySkeletonAddress}`} />
            </TableCell>
            <TableCell className={styles.valueContainer}>
              <div className={`${styles.skeleton} ${styles.historySkeletonValue}`} />
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}

function ContractCodeSkeleton(): JSX.Element {
  return (
    <div className={styles.contractSkeleton} aria-label="Loading contract code">
      <div className={`${styles.skeleton} ${styles.contractSkeletonTabs}`} />
      <div className={`${styles.skeleton} ${styles.contractSkeletonBlock}`} />
    </div>
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
    const hiddenToncenterActionKeys = Array.isArray(parsed.hiddenToncenterActionKeys)
      ? parsed.hiddenToncenterActionKeys.filter(
          (value): value is string => typeof value === "string",
        )
      : []
    const historyMode: AccountHistoryMode =
      parsed.historyMode === "transactions" ? "transactions" : "actions"
    const sortOrder: AccountSortOrder = parsed.sortOrder === "asc" ? "asc" : "desc"
    const timeFormat: AccountTimeFormat =
      parsed.timeFormat === "relative" ||
      parsed.timeFormat === "smart" ||
      parsed.timeFormat === "absolute"
        ? parsed.timeFormat
        : "smart"

    return {historyMode, hiddenActionKeys, hiddenToncenterActionKeys, sortOrder, timeFormat}
  } catch {
    return DEFAULT_TRANSACTION_FILTERS
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function HistoryTechnicalCell({
  technicalLabel,
}: {
  readonly technicalLabel?: HistoryTechnicalLabel
}): JSX.Element | null {
  if (!technicalLabel) {
    return null
  }

  return (
    <span className={styles.technicalLabel} title={technicalLabel.label}>
      {technicalLabel.label}
    </span>
  )
}

function HistoryValueCellLine({line}: {readonly line: HistoryValueLine}): JSX.Element {
  if (line.kind === "swap") {
    return (
      <div className={`${styles.swapValue} ${styles.historyValue}`}>
        <span className={`${historyValueToneClass(line.from.tone)} ${styles.swapValueSegment}`}>
          {line.from.label}
        </span>
        <ChevronRight className={styles.swapValueArrow} aria-hidden="true" />
        <span className={`${historyValueToneClass(line.to.tone)} ${styles.swapValueSegment}`}>
          {line.to.label}
        </span>
      </div>
    )
  }

  return (
    <div className={`${historyValueToneClass(line.tone)} ${styles.historyValue}`}>{line.label}</div>
  )
}

interface ActionHistoryRowsProps {
  readonly rows: readonly HistoryActionRow[]
  readonly nowSeconds: number
  readonly timeFormat: AccountTimeFormat
  readonly highlightedTransactionHashSet?: ReadonlySet<string>
  readonly showTimeColumn?: boolean
  readonly interactiveRows?: boolean
  readonly onAddressClick?: (addr: string, event?: MouseEvent<HTMLElement>) => void
  readonly onActionHoverChange?: (action: V3Action | undefined) => void
  readonly onTransactionClick?: (hash: string, event?: MouseEvent<HTMLElement>) => void
}

export function ActionHistoryRows({
  rows,
  nowSeconds,
  timeFormat,
  highlightedTransactionHashSet,
  showTimeColumn = true,
  interactiveRows = true,
  onAddressClick,
  onActionHoverChange,
  onTransactionClick,
}: ActionHistoryRowsProps): JSX.Element {
  const [hoveredAddress, setHoveredAddress] = useState<string | undefined>()

  return (
    <>
      {rows.map(({action, info}, index) => {
        const formattedTime = showTimeColumn
          ? formatTransactionTime(info.utime, nowSeconds, timeFormat)
          : undefined
        const isAddressHovered =
          hoveredAddress && info.address ? isSameAddress(info.address, hoveredAddress) : false
        const isHighlighted =
          highlightedTransactionHashSet !== undefined &&
          info.transactionHashes.some(hash => highlightedTransactionHashSet.has(hash))
        const canOpenTransaction =
          interactiveRows && info.transactionHash !== undefined && onTransactionClick !== undefined
        const ActionIcon = getHistoryActionIcon(action, info)
        const continuesTrace = isSameActionTrace(action, rows[index + 1]?.action)
        const continuesFromTrace = isSameActionTrace(rows[index - 1]?.action, action)

        return (
          <TableRow
            key={info.rowKey}
            className={`${styles.row} ${interactiveRows ? "" : styles.rowStatic} ${
              canOpenTransaction ? styles.clickableRow : ""
            } ${isHighlighted ? styles.newTransactionRow : ""} ${
              continuesTrace ? styles.actionChainContinues : ""
            } ${continuesFromTrace ? styles.actionChainContinuation : ""}`}
            onClick={
              canOpenTransaction
                ? event => {
                    if (info.transactionHash) onTransactionClick(info.transactionHash, event)
                  }
                : undefined
            }
            onMouseEnter={onActionHoverChange ? () => onActionHoverChange(action) : undefined}
            onMouseLeave={onActionHoverChange ? () => onActionHoverChange(undefined) : undefined}
          >
            {showTimeColumn && (
              <TableCell className={`${styles.time} ${styles.timeColumn}`}>
                {!continuesFromTrace && formattedTime && (
                  <span title={formattedTime.title}>{formattedTime.label}</span>
                )}
              </TableCell>
            )}
            <TableCell className={styles.actionColumn}>
              <div className={styles.action}>
                <ActionIcon className={styles.actionIcon} aria-hidden="true" />
                <span
                  className={`${styles.actionText} ${styles.opcode}`}
                  title={action.type ?? info.actionLabel}
                >
                  {info.actionLabel}
                </span>
              </div>
            </TableCell>
            <TableCell>
              <div className={styles.addressWrapper}>
                {info.relationLabel && (
                  <span className={styles.addressRelation}>{info.relationLabel}</span>
                )}
                {info.address ? (
                  <AddressChip
                    address={info.address}
                    fallback={info.displayAddressFallback}
                    highlighted={isAddressHovered}
                    onAddressClick={onAddressClick}
                    onHoverAddressChange={setHoveredAddress}
                  />
                ) : (
                  <span className={styles.addressFallback}>{info.displayAddressFallback}</span>
                )}
              </div>
            </TableCell>
            <TableCell className={styles.technicalColumn}>
              <HistoryTechnicalCell technicalLabel={info.technicalLabel} />
            </TableCell>
            <TableCell className={styles.valueContainer}>
              <div className={styles.historyValueStack}>
                {info.valueLines.map((line, lineIndex) => (
                  <HistoryValueCellLine key={`${info.rowKey}:value:${lineIndex}`} line={line} />
                ))}
              </div>
            </TableCell>
          </TableRow>
        )
      })}
    </>
  )
}

interface ActionHistoryTableProps {
  readonly actions: readonly V3Action[]
  readonly actionMetadata?: V3Metadata
  readonly ownerAddress: string
  readonly client: TonClient
  readonly nowSeconds: number
  readonly timeFormat?: AccountTimeFormat
  readonly emptyState?: string
  readonly className?: string
  readonly showTimeColumn?: boolean
  readonly interactiveRows?: boolean
  readonly onAddressClick?: (addr: string, event?: MouseEvent<HTMLElement>) => void
  readonly onActionHoverChange?: (action: V3Action | undefined) => void
  readonly onTransactionClick?: (hash: string, event?: MouseEvent<HTMLElement>) => void
}

export function ActionHistoryTable({
  actions,
  actionMetadata = {},
  ownerAddress,
  client,
  nowSeconds,
  timeFormat = "smart",
  emptyState = "No actions found",
  className,
  showTimeColumn = true,
  interactiveRows = true,
  onAddressClick,
  onActionHoverChange,
  onTransactionClick,
}: ActionHistoryTableProps): JSX.Element {
  const actionAddresses = useMemo(() => collectActionMessageNameAddresses(actions), [actions])
  const messageNamesByAddress = useMessageNamesByAddress({
    client,
    addresses: actionAddresses,
  })
  const rows = useMemo(
    () => buildHistoryActionRows(actions, ownerAddress, actionMetadata, messageNamesByAddress),
    [actions, actionMetadata, messageNamesByAddress, ownerAddress],
  )

  return (
    <div className={`${styles.historyContent} ${className ?? ""}`}>
      <Table>
        <TableHeader className={styles.historyHeaderGroup}>
          <TableRow className={styles.historyHeaderRow}>
            {showTimeColumn && (
              <TableHead className={`${styles.tableHeader} ${styles.timeColumn}`}>Time</TableHead>
            )}
            <TableHead className={`${styles.tableHeader} ${styles.actionColumn}`}>Action</TableHead>
            <TableHead className={styles.tableHeader}>Address</TableHead>
            <TableHead className={`${styles.tableHeader} ${styles.technicalColumn}`} />
            <TableHead className={`${styles.tableHeader} ${styles.valueContainer}`}>
              Value
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {rows.length === 0 ? (
            <TableRow className={styles.emptyRow}>
              <TableCell colSpan={showTimeColumn ? 5 : 4} className={styles.emptyCell}>
                <div className={styles.tableState}>{emptyState}</div>
              </TableCell>
            </TableRow>
          ) : (
            <ActionHistoryRows
              rows={rows}
              nowSeconds={nowSeconds}
              timeFormat={timeFormat}
              showTimeColumn={showTimeColumn}
              interactiveRows={interactiveRows}
              onAddressClick={onAddressClick}
              onActionHoverChange={onActionHoverChange}
              onTransactionClick={onTransactionClick}
            />
          )}
        </TableBody>
      </Table>
    </div>
  )
}

function getHistoryTransactionInfo(
  tx: V3TransactionListItem,
  browsedAddr: ReturnType<typeof parseAddress>,
  messageNamesByAddress: MessageNamesByAddress,
): HistoryTransactionInfo {
  const inMsg = tx.in_msg
  const outMsgs = tx.out_msgs
  if (!inMsg && outMsgs.length === 0) {
    return {
      isIncoming: false,
      address: "",
      displayAddressFallback: "System",
      actionKey: "system:tick-tock",
      actionLabel: "Tick-tock",
      displayValue: BigInt(0),
    }
  }

  const inMsgSrc = parseAddress(inMsg?.source || "")
  const inMsgDest = parseAddress(inMsg?.destination || "")
  const isInboundToAccount = inMsgDest && browsedAddr ? inMsgDest.equals(browsedAddr) : false
  const isIncoming =
    isInboundToAccount &&
    browsedAddr !== undefined &&
    inMsgSrc !== undefined &&
    (!inMsgSrc.equals(browsedAddr) || outMsgs.length === 0)

  const inValue = BigInt(inMsg?.value || "0")
  const outValue = outMsgs.reduce((acc, msg) => acc + BigInt(msg.value || "0"), BigInt(0))
  const displayValue = isIncoming ? inValue : outValue
  const address = isIncoming
    ? inMsg?.source || ""
    : outMsgs.find(message => message.destination)?.destination || ""
  const displayAddressFallback = isIncoming ? "External" : "Contract"
  const displayMessage = isIncoming
    ? (inMsg ?? undefined)
    : outMsgs.find(message => message.destination) ||
      outMsgs.find(message => message.opcode) ||
      outMsgs[0]
  const opcode = normalizeOpcode(displayMessage?.opcode)
  const normalizedOpcode = opcode
  const actionLabel =
    resolveMessageName(displayMessage, messageNamesByAddress) ||
    opcode ||
    (isIncoming ? "Received GRAM" : "Send GRAM")
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

const ACTION_TYPE_LABELS = {
  auction_bid: "Auction bid",
  auction_outbid: "Auction outbid",
  call_contract: "Called contract",
  change_dns: "Change DNS",
  cocoon_client_change_secret_hash: "Change secret hash",
  cocoon_client_increase_stake: "Increase stake",
  cocoon_client_register: "Register client",
  cocoon_client_request_refund: "Request refund",
  cocoon_client_top_up: "Top up",
  cocoon_client_withdraw: "Withdraw",
  cocoon_grant_refund: "Grant refund",
  cocoon_proxy_charge: "Proxy charge",
  cocoon_proxy_payout: "Proxy payout",
  cocoon_register_proxy: "Register proxy",
  cocoon_unregister_proxy: "Unregister proxy",
  cocoon_worker_payout: "Worker payout",
  coffee_create_pool: "Create pool",
  coffee_create_pool_creator: "Create pool",
  coffee_create_vault: "Create vault",
  coffee_mev_protect_hold_funds: "Hold funds",
  coffee_staking_claim_rewards: "Claim rewards",
  coffee_staking_deposit: "Deposit stake",
  coffee_staking_withdraw: "Withdraw stake",
  contract_deploy: "Contract deploy",
  delete_dns: "Delete DNS",
  dex_deposit_liquidity: "Deposit liquidity",
  dex_withdraw_liquidity: "Withdraw liquidity",
  dns_purchase: "DNS purchase",
  dns_release: "DNS release",
  election_deposit: "Election deposit",
  election_recover: "Election recover",
  evaa_liquidate: "EVAA liquidate",
  evaa_supply: "EVAA supply",
  evaa_withdraw: "EVAA withdraw",
  extra_currency_transfer: "Extra currency transfer",
  jetton_burn: "Burn token",
  jetton_mint: "Mint token",
  jetton_swap: "Swap tokens",
  jetton_transfer: "Token transfer",
  jvault_claim: "Claim rewards",
  jvault_stake: "Stake",
  jvault_unstake: "Unstake",
  jvault_unstake_request: "Unstake request",
  layerzero_commit_packet: "LayerZero commit packet",
  layerzero_dvn_verify: "LayerZero verify",
  layerzero_receive: "LayerZero receive",
  layerzero_send: "LayerZero send",
  layerzero_send_tokens: "LayerZero send tokens",
  multisig_approve: "Multisig approve",
  multisig_create_order: "Multisig create order",
  multisig_execute: "Multisig execute",
  nft_cancel_auction: "Cancel auction",
  nft_cancel_sale: "Cancel sale",
  nft_discovery: "NFT discovery",
  nft_finish_auction: "Finish auction",
  nft_mint: "Mint NFT",
  nft_purchase: "NFT purchase",
  nft_put_on_auction: "Put on auction",
  nft_put_on_sale: "Put on sale",
  nft_transfer: "NFT transfer",
  nft_update_sale: "Update sale",
  renew_dns: "Renew DNS",
  stake_deposit: "Deposit stake",
  stake_withdrawal: "Withdraw stake",
  stake_withdrawal_request: "Withdraw stake request",
  subscribe: "Subscribe",
  teleitem_cancel_auction: "Cancel auction",
  teleitem_start_auction: "Start auction",
  tgbtc_burn: "tgBTC burn",
  tgbtc_burn_fallback: "tgBTC burn",
  tgbtc_dkg_log_fallback: "tgBTC DKG log",
  tgbtc_mint: "tgBTC mint",
  tgbtc_mint_fallback: "tgBTC mint",
  tgbtc_new_key: "tgBTC new key",
  tgbtc_new_key_fallback: "tgBTC new key",
  tick_tock: "Tick-tock",
  ton_transfer: "Transfer",
  tonco_deploy_pool: "Deploy pool",
  tonco_jetton_swap: "Swap tokens",
  unsubscribe: "Unsubscribe",
  vesting_add_whitelist: "Add whitelist",
  vesting_send_message: "Vesting send message",
  wton_mint: "Mint wTON",
} satisfies Readonly<Record<V3Action["type"], string>>

const ACTION_TYPE_ICONS = {
  auction_bid: Gavel,
  auction_outbid: Gavel,
  call_contract: Code2,
  change_dns: Globe2,
  cocoon_client_change_secret_hash: KeyRound,
  cocoon_client_increase_stake: Pickaxe,
  cocoon_client_register: ServerCog,
  cocoon_client_request_refund: BadgeMinus,
  cocoon_client_top_up: BadgePlus,
  cocoon_client_withdraw: BadgeMinus,
  cocoon_grant_refund: BadgeMinus,
  cocoon_proxy_charge: ServerCog,
  cocoon_proxy_payout: ServerCog,
  cocoon_register_proxy: ServerCog,
  cocoon_unregister_proxy: ServerCog,
  cocoon_worker_payout: ServerCog,
  coffee_create_pool: Layers,
  coffee_create_pool_creator: Layers,
  coffee_create_vault: Vault,
  coffee_mev_protect_hold_funds: ShieldCheck,
  coffee_staking_claim_rewards: BadgeDollarSign,
  coffee_staking_deposit: Landmark,
  coffee_staking_withdraw: Landmark,
  contract_deploy: FileCode2,
  delete_dns: Globe2,
  dex_deposit_liquidity: Layers,
  dex_withdraw_liquidity: Layers,
  dns_purchase: Globe2,
  dns_release: Globe2,
  election_deposit: ShieldCheck,
  election_recover: ShieldCheck,
  evaa_liquidate: Landmark,
  evaa_supply: Landmark,
  evaa_withdraw: Landmark,
  extra_currency_transfer: WalletCards,
  jetton_burn: Flame,
  jetton_mint: BadgePlus,
  jetton_swap: RefreshCw,
  jetton_transfer: Coins,
  jvault_claim: BadgeDollarSign,
  jvault_stake: Landmark,
  jvault_unstake: Landmark,
  jvault_unstake_request: Landmark,
  layerzero_commit_packet: Network,
  layerzero_dvn_verify: ShieldCheck,
  layerzero_receive: Webhook,
  layerzero_send: Webhook,
  layerzero_send_tokens: Webhook,
  multisig_approve: KeyRound,
  multisig_create_order: SquareStack,
  multisig_execute: KeyRound,
  nft_cancel_auction: Gavel,
  nft_cancel_sale: ImageIcon,
  nft_discovery: ImageIcon,
  nft_finish_auction: Gavel,
  nft_mint: ImagePlus,
  nft_purchase: ImageIcon,
  nft_put_on_auction: Gavel,
  nft_put_on_sale: ImageIcon,
  nft_transfer: ImageIcon,
  nft_update_sale: ImageIcon,
  renew_dns: Globe2,
  stake_deposit: Landmark,
  stake_withdrawal: Landmark,
  stake_withdrawal_request: Landmark,
  subscribe: Bell,
  teleitem_cancel_auction: Gavel,
  teleitem_start_auction: Gavel,
  tgbtc_burn: Flame,
  tgbtc_burn_fallback: Flame,
  tgbtc_dkg_log_fallback: Database,
  tgbtc_mint: BadgePlus,
  tgbtc_mint_fallback: BadgePlus,
  tgbtc_new_key: KeyRound,
  tgbtc_new_key_fallback: KeyRound,
  tick_tock: CircleDot,
  ton_transfer: WalletCards,
  tonco_deploy_pool: PackagePlus,
  tonco_jetton_swap: RefreshCw,
  unsubscribe: Bell,
  vesting_add_whitelist: LockKeyhole,
  vesting_send_message: LockKeyhole,
  wton_mint: BadgePlus,
} satisfies Readonly<Record<V3Action["type"], LucideIcon>>

const EMPTY_VALUE_LINES: readonly HistoryValueLine[] = [
  {kind: "text", label: "empty", tone: "empty"},
]

interface HistoryActionDisplay {
  readonly isIncoming: boolean
  readonly address: string
  readonly displayAddressFallback: string
  readonly relationLabel?: string
  readonly valueLines: readonly HistoryValueLine[]
}

interface HistoryActionRenderContext {
  readonly metadata: V3Metadata
  readonly ownerAddress: string
}

function getHistoryActionInfo(
  action: V3Action,
  ownerAddress: string,
  metadata: V3Metadata,
  messageNamesByAddress: MessageNamesByAddress,
  fallbackIndex: number,
): HistoryActionInfo {
  const display = getHistoryActionDisplay(action, {metadata, ownerAddress})
  const transactionHashes = action.transactions.filter(isNonEmptyString)
  const transactionHash = transactionHashes.map(hashToHex).find(isNonEmptyString)

  return {
    rowKey: getActionRowKey(action, fallbackIndex),
    transactionHash,
    transactionHashes,
    utime: action.end_utime || action.trace_end_utime || action.start_utime,
    isIncoming: display.isIncoming,
    success: action.success !== false,
    address: display.address,
    displayAddressFallback: display.displayAddressFallback,
    relationLabel: display.relationLabel,
    actionKey: `toncenter:${action.type}`,
    actionLabel: getHistoryActionLabel(action, display.isIncoming),
    technicalLabel: getHistoryActionTechnicalLabel(action, messageNamesByAddress),
    valueLines: display.valueLines,
  }
}

function buildHistoryActionRows(
  actions: readonly V3Action[],
  ownerAddress: string,
  metadata: V3Metadata,
  messageNamesByAddress: MessageNamesByAddress,
): readonly HistoryActionRow[] {
  return actions.map((action, index) => ({
    action,
    info: getHistoryActionInfo(action, ownerAddress, metadata, messageNamesByAddress, index),
  }))
}

function getHistoryActionLabel(action: V3Action, isIncoming: boolean): string {
  if (action.type === "ton_transfer") {
    return isIncoming ? "Received GRAM" : "Send GRAM"
  }

  return ACTION_TYPE_LABELS[action.type] ?? "Unsupported action"
}

function getHistoryActionIcon(action: V3Action, info: HistoryActionInfo): LucideIcon {
  if (!info.success) {
    return CircleX
  }

  if (action.type === "ton_transfer") {
    return info.isIncoming ? MoveDownLeft : MoveUpRight
  }

  return ACTION_TYPE_ICONS[action.type] ?? CircleDot
}

function getActionRowKey(action: V3Action, fallbackIndex: number): string {
  if (isNonEmptyString(action.action_id)) {
    return action.action_id
  }

  return [
    action.trace_id,
    action.type,
    action.start_lt,
    action.end_lt,
    action.transactions.join(":"),
    `row-${fallbackIndex}`,
  ]
    .filter(isNonEmptyString)
    .join(":")
}

function isSameActionTrace(left: V3Action | undefined, right: V3Action | undefined): boolean {
  return isNonEmptyString(left?.trace_id) && left.trace_id === right?.trace_id
}

function collectActionMessageNameAddresses(actions: readonly V3Action[]): string[] {
  const addresses = new Set<string>()

  for (const action of actions) {
    for (const account of action.accounts ?? []) {
      addHistoryAddress(addresses, account)
    }

    switch (action.type) {
      case "call_contract":
      case "contract_deploy":
        addHistoryAddress(addresses, action.details.source)
        addHistoryAddress(addresses, action.details.destination)
        break
      case "extra_currency_transfer":
        addHistoryAddress(addresses, action.details.source)
        addHistoryAddress(addresses, action.details.destination)
        break
      default:
        break
    }
  }

  return [...addresses]
}

function addHistoryAddress(addresses: Set<string>, address: string | null | undefined): void {
  if (isNonEmptyString(address)) {
    addresses.add(address)
  }
}

function getHistoryActionTechnicalLabel(
  action: V3Action,
  messageNamesByAddress: MessageNamesByAddress,
): HistoryTechnicalLabel | undefined {
  switch (action.type) {
    case "call_contract":
    case "contract_deploy":
      return opcodeTechnicalLabel(
        action.details.opcode,
        action.details.source,
        action.details.destination,
        messageNamesByAddress,
      )
    case "extra_currency_transfer":
      if ("comment" in action.details) {
        return commentTechnicalLabel(action.details.comment, action.details.encrypted)
      }
      return opcodeTechnicalLabel(
        action.details.opcode,
        action.details.source,
        action.details.destination,
        messageNamesByAddress,
      )
    case "ton_transfer":
      return commentTechnicalLabel(action.details.comment, action.details.encrypted)
    case "jetton_transfer":
      return commentTechnicalLabel(action.details.comment, action.details.is_encrypted_comment)
    case "nft_transfer":
    case "nft_purchase":
      return commentTechnicalLabel(action.details.comment, action.details.is_encrypted_comment)
    case "auction_outbid":
      return commentTechnicalLabel(action.details.comment, false)
    default:
      return undefined
  }
}

function opcodeTechnicalLabel(
  opcode: string | number | null | undefined,
  source: string | null | undefined,
  destination: string | null | undefined,
  messageNamesByAddress: MessageNamesByAddress,
): HistoryTechnicalLabel | undefined {
  const normalizedOpcode = normalizeOpcode(opcode)
  if (!normalizedOpcode) {
    return undefined
  }

  const resolvedName = resolveOpcodeName(
    normalizedOpcode,
    source,
    destination,
    messageNamesByAddress,
  )
  return {label: resolvedName ?? normalizedOpcode}
}

function commentTechnicalLabel(
  comment: string | null | undefined,
  encrypted: boolean | null | undefined,
): HistoryTechnicalLabel | undefined {
  if (encrypted === true || !isNonEmptyString(comment)) {
    return undefined
  }

  const label = comment.trim()
  return label ? {label} : undefined
}

function getHistoryActionDisplay(
  action: V3Action,
  context: HistoryActionRenderContext,
): HistoryActionDisplay {
  switch (action.type) {
    case "call_contract":
    case "extra_currency_transfer":
    case "ton_transfer":
      return sourceDestinationAction(
        action.details.source ?? null,
        action.details.destination ?? null,
        context.ownerAddress,
        isIncoming =>
          valueLines(
            tonValueLine(action.details.value ?? null, isIncoming ? "positive" : "negative"),
          ),
        "Account",
      )
    case "contract_deploy": {
      const destination = action.details.destination ?? null
      const isIncoming = destination ? isSameAddress(destination, context.ownerAddress) : false
      return {
        isIncoming,
        address: destination ?? "",
        displayAddressFallback: "Contract",
        relationLabel: destination ? "to" : undefined,
        valueLines: valueLines(
          tonValueLine(action.details.value ?? null, isIncoming ? "positive" : "negative"),
        ),
      }
    }
    case "auction_bid":
      return sourceDestinationAction(
        action.details.bidder,
        action.details.auction,
        context.ownerAddress,
        () => valueLines(tonValueLine(action.details.amount, "negative")),
        "Auction",
      )
    case "auction_outbid":
      return addressAction(
        action.details.auction_address,
        "on",
        false,
        "Auction",
        valueLines(tonValueLine(action.details.amount, "neutral")),
      )
    case "change_dns":
      return sourceDestinationAction(
        action.details.source,
        action.details.asset,
        context.ownerAddress,
        () => valueLines(textValueLine(action.details.key, "neutral")),
        "DNS",
      )
    case "delete_dns":
      return sourceDestinationAction(
        action.details.source,
        action.details.asset,
        context.ownerAddress,
        () => valueLines(textValueLine(action.details.hash, "neutral")),
        "DNS",
      )
    case "renew_dns":
      return sourceDestinationAction(
        action.details.source,
        action.details.asset,
        context.ownerAddress,
        () => EMPTY_VALUE_LINES,
        "DNS",
      )
    case "dns_purchase":
      return addressAction(
        action.details.nft_item,
        "item",
        true,
        "DNS",
        valueLines(
          tonValueLine(action.details.price, "negative"),
          nftValueLine(action.details.nft_item, action.details.nft_item_index, context.metadata),
        ),
      )
    case "dns_release":
      return sourceDestinationAction(
        action.details.source,
        action.details.nft_item,
        context.ownerAddress,
        isIncoming =>
          valueLines(tonValueLine(action.details.value, isIncoming ? "positive" : "negative")),
        "DNS",
      )
    case "election_deposit":
      return addressAction(
        action.details.stake_holder,
        "holder",
        false,
        "Validator",
        valueLines(tonValueLine(action.details.amount ?? null, "negative")),
      )
    case "election_recover":
      return addressAction(
        action.details.stake_holder,
        "holder",
        true,
        "Validator",
        valueLines(tonValueLine(action.details.amount ?? null, "positive")),
      )
    case "jetton_transfer":
      return sourceDestinationAction(
        action.details.sender,
        action.details.receiver,
        context.ownerAddress,
        isIncoming =>
          valueLines(
            assetValueLine(
              action.details.amount,
              action.details.asset,
              context.metadata,
              isIncoming ? "positive" : "negative",
            ),
          ),
        "Account",
        {
          destinationAccounts: [action.details.receiver_jetton_wallet],
          sourceAccounts: [action.details.sender_jetton_wallet],
        },
      )
    case "jetton_mint":
      return addressAction(
        action.details.asset,
        "asset",
        true,
        "Jetton",
        valueLines(
          assetValueLine(action.details.amount, action.details.asset, context.metadata, "positive"),
        ),
      )
    case "jetton_burn":
      return addressAction(
        action.details.asset,
        "asset",
        false,
        "Jetton",
        valueLines(
          assetValueLine(action.details.amount, action.details.asset, context.metadata, "negative"),
        ),
      )
    case "jetton_swap":
    case "tonco_jetton_swap":
      return addressAction(
        action.details.dex_outgoing_transfer?.source ??
          action.details.dex_incoming_transfer?.destination ??
          null,
        "on",
        false,
        "DEX",
        valueLines(
          swapValueLine(
            action.details.dex_incoming_transfer?.amount ?? null,
            action.details.dex_incoming_transfer?.asset ?? action.details.asset_in,
            action.details.dex_outgoing_transfer?.amount ?? null,
            action.details.dex_outgoing_transfer?.asset ?? action.details.asset_out,
            context.metadata,
          ),
        ),
      )
    case "nft_mint":
      return addressAction(
        action.details.nft_item,
        "item",
        true,
        "NFT",
        valueLines(
          nftValueLine(action.details.nft_item, action.details.nft_item_index, context.metadata),
        ),
      )
    case "nft_transfer":
    case "nft_purchase":
      return sourceDestinationAction(
        action.details.old_owner ?? null,
        action.details.new_owner,
        context.ownerAddress,
        isIncoming =>
          action.type === "nft_purchase"
            ? valueLines(
                tonValueLine(action.details.price ?? null, isIncoming ? "negative" : "positive"),
                nftValueLine(
                  action.details.nft_item,
                  action.details.nft_item_index,
                  context.metadata,
                ),
              )
            : valueLines(
                nftValueLine(
                  action.details.nft_item,
                  action.details.nft_item_index,
                  context.metadata,
                ),
              ),
        "Account",
      )
    case "nft_put_on_sale":
      return addressAction(
        action.details.sale_address,
        "sale",
        false,
        "Sale",
        valueLines(tonValueLine(action.details.full_price, "neutral")),
      )
    case "nft_put_on_auction":
    case "teleitem_start_auction":
      return addressAction(
        action.details.auction_address,
        "auction",
        false,
        "Auction",
        valueLines(
          tonValueLine(action.details.min_bid, "neutral"),
          tonValueLine(action.details.max_bid, "neutral"),
        ),
      )
    case "nft_cancel_sale":
      return addressAction(
        action.details.sale_address,
        "sale",
        false,
        "Sale",
        valueLines(nftValueLine(action.details.nft_item, null, context.metadata)),
      )
    case "nft_cancel_auction":
    case "teleitem_cancel_auction":
    case "nft_finish_auction":
      return addressAction(
        action.details.auction_address,
        "auction",
        false,
        "Auction",
        valueLines(nftValueLine(action.details.nft_item, null, context.metadata)),
      )
    case "nft_update_sale":
      return addressAction(
        action.details.sale_contract,
        "sale",
        false,
        "Sale",
        valueLines(tonValueLine(action.details.full_price, "neutral")),
      )
    case "nft_discovery":
      return addressAction(
        action.details.nft_item,
        "item",
        true,
        "NFT",
        valueLines(
          nftValueLine(action.details.nft_item, action.details.nft_item_index, context.metadata),
        ),
      )
    case "tick_tock":
      return addressAction(
        action.details.account ?? null,
        "account",
        false,
        "System",
        EMPTY_VALUE_LINES,
      )
    case "stake_deposit":
      return addressAction(
        action.details.pool,
        "to",
        false,
        "Pool",
        valueLines(
          assetOrTonValueLine(
            action.details.amount,
            action.details.asset,
            context.metadata,
            "negative",
          ),
        ),
      )
    case "stake_withdrawal":
      return addressAction(
        action.details.pool,
        "to",
        true,
        "Pool",
        valueLines(
          assetOrTonValueLine(
            action.details.amount,
            action.details.asset,
            context.metadata,
            "positive",
          ),
        ),
      )
    case "stake_withdrawal_request":
      return addressAction(
        action.details.pool,
        "to",
        false,
        "Pool",
        valueLines(
          assetValueLine(
            action.details.tokens_burnt,
            action.details.asset,
            context.metadata,
            "negative",
          ),
        ),
      )
    case "subscribe":
      return sourceDestinationAction(
        action.details.subscriber,
        action.details.beneficiary ?? action.details.subscription,
        context.ownerAddress,
        () => valueLines(tonValueLine(action.details.amount, "negative")),
        "Subscription",
      )
    case "unsubscribe":
      return sourceDestinationAction(
        action.details.subscriber,
        action.details.beneficiary ?? action.details.subscription,
        context.ownerAddress,
        () => valueLines(tonValueLine(action.details.amount ?? null, "positive")),
        "Subscription",
      )
    case "wton_mint":
      return addressAction(
        action.details.receiver,
        "receiver",
        true,
        "Account",
        valueLines(assetValueLine(action.details.amount, null, context.metadata, "positive")),
      )
    case "dex_deposit_liquidity":
      return addressAction(
        action.details.pool,
        "on",
        false,
        "Pool",
        valueLines(
          assetValueLine(
            action.details.amount_1,
            action.details.asset_1,
            context.metadata,
            "negative",
          ),
          assetValueLine(
            action.details.amount_2,
            action.details.asset_2,
            context.metadata,
            "negative",
          ),
        ),
      )
    case "dex_withdraw_liquidity":
      return addressAction(
        action.details.pool,
        "on",
        true,
        "Pool",
        valueLines(
          assetValueLine(
            action.details.amount_1,
            action.details.asset_1,
            context.metadata,
            "positive",
          ),
          assetValueLine(
            action.details.amount_2,
            action.details.asset_2,
            context.metadata,
            "positive",
          ),
        ),
      )
    case "tonco_deploy_pool":
      return sourceDestinationAction(
        action.details.source,
        action.details.pool,
        context.ownerAddress,
        () => EMPTY_VALUE_LINES,
        "Pool",
      )
    case "multisig_create_order":
      return sourceDestinationAction(
        action.details.source,
        action.details.destination,
        context.ownerAddress,
        () => EMPTY_VALUE_LINES,
        "Multisig",
      )
    case "multisig_approve":
    case "multisig_execute":
      return sourceDestinationAction(
        action.details.source,
        action.details.destination,
        context.ownerAddress,
        () => EMPTY_VALUE_LINES,
        "Multisig",
      )
    case "vesting_send_message":
      return sourceDestinationAction(
        action.details.source,
        action.details.destination,
        context.ownerAddress,
        isIncoming =>
          valueLines(tonValueLine(action.details.amount, isIncoming ? "positive" : "negative")),
        "Vesting",
      )
    case "vesting_add_whitelist":
      return sourceDestinationAction(
        action.details.source,
        action.details.vesting,
        context.ownerAddress,
        () => EMPTY_VALUE_LINES,
        "Vesting",
      )
    case "evaa_supply":
      return sourceDestinationAction(
        action.details.source,
        action.details.recipient_contract,
        context.ownerAddress,
        () =>
          valueLines(
            assetOrTonValueLine(
              action.details.amount,
              action.details.asset,
              context.metadata,
              "negative",
            ),
          ),
        "EVAA",
        {
          destinationAccounts: [action.details.recipient_jetton_wallet],
          sourceAccounts: [action.details.source_wallet, action.details.sender_jetton_wallet],
        },
      )
    case "evaa_withdraw":
      return sourceDestinationAction(
        action.details.source,
        action.details.recipient,
        context.ownerAddress,
        isIncoming =>
          valueLines(
            assetOrTonValueLine(
              action.details.amount,
              action.details.asset,
              context.metadata,
              isIncoming ? "positive" : "negative",
            ),
          ),
        "EVAA",
        {
          destinationAccounts: [action.details.recipient_jetton_wallet],
          sourceAccounts: [action.details.owner_contract],
        },
      )
    case "evaa_liquidate":
      return sourceDestinationAction(
        action.details.source,
        action.details.borrower,
        context.ownerAddress,
        () =>
          valueLines(
            assetOrTonValueLine(
              action.details.amount,
              action.details.asset,
              context.metadata,
              "neutral",
            ),
          ),
        "EVAA",
      )
    case "jvault_claim":
      return addressAction(
        action.details.pool,
        "to",
        true,
        "Pool",
        valueLines(
          ...action.details.claimed_rewards.map(reward =>
            assetValueLine(reward.amount, reward.jetton, context.metadata, "positive"),
          ),
        ),
      )
    case "jvault_stake":
      return addressAction(
        action.details.pool,
        "to",
        false,
        "Pool",
        valueLines(
          assetValueLine(action.details.amount, action.details.asset, context.metadata, "negative"),
        ),
      )
    case "jvault_unstake":
      return addressAction(
        action.details.pool,
        "to",
        true,
        "Pool",
        valueLines(
          assetValueLine(action.details.amount, action.details.asset, context.metadata, "positive"),
        ),
      )
    case "jvault_unstake_request":
      return addressAction(
        action.details.pool,
        "to",
        false,
        "Pool",
        valueLines(
          assetValueLine(action.details.amount, action.details.asset, context.metadata, "negative"),
        ),
      )
    case "tgbtc_mint":
    case "tgbtc_mint_fallback":
      return sourceDestinationAction(
        action.details.source,
        action.details.destination,
        context.ownerAddress,
        isIncoming =>
          valueLines(
            assetValueLine(
              action.details.amount,
              action.details.asset,
              context.metadata,
              isIncoming ? "positive" : "negative",
            ),
          ),
        "tgBTC",
        {
          destinationAccounts: [action.details.destination_wallet],
        },
      )
    case "tgbtc_burn":
    case "tgbtc_burn_fallback":
      return sourceDestinationAction(
        action.details.source,
        action.details.destination,
        context.ownerAddress,
        () =>
          valueLines(
            assetValueLine(
              action.details.amount,
              action.details.asset,
              context.metadata,
              "negative",
            ),
          ),
        "tgBTC",
        {
          sourceAccounts: [action.details.source_wallet],
        },
      )
    case "tgbtc_new_key":
    case "tgbtc_new_key_fallback":
      return sourceDestinationAction(
        action.details.source,
        action.details.coordinator,
        context.ownerAddress,
        () =>
          valueLines(
            assetValueLine(
              action.details.amount,
              action.details.asset,
              context.metadata,
              "neutral",
            ),
          ),
        "tgBTC",
      )
    case "tgbtc_dkg_log_fallback":
      return addressAction(
        action.details.coordinator,
        "coordinator",
        false,
        "tgBTC",
        EMPTY_VALUE_LINES,
      )
    case "coffee_create_pool":
      return addressAction(
        action.details.pool,
        "on",
        false,
        "Pool",
        valueLines(
          assetValueLine(
            action.details.amount_1,
            action.details.asset_1,
            context.metadata,
            "negative",
          ),
          assetValueLine(
            action.details.amount_2,
            action.details.asset_2,
            context.metadata,
            "negative",
          ),
        ),
      )
    case "coffee_create_pool_creator":
      return sourceDestinationAction(
        action.details.source,
        action.details.pool_creator_contract,
        context.ownerAddress,
        () =>
          valueLines(
            assetValueLine(
              action.details.amount,
              action.details.provided_asset,
              context.metadata,
              "negative",
            ),
          ),
        "Pool",
      )
    case "coffee_staking_deposit":
      return addressAction(
        action.details.pool,
        "to",
        false,
        "Pool",
        valueLines(
          assetValueLine(action.details.amount, action.details.asset, context.metadata, "negative"),
        ),
      )
    case "coffee_staking_withdraw":
      return addressAction(
        action.details.pool,
        "to",
        true,
        "Pool",
        valueLines(
          assetValueLine(action.details.amount, action.details.asset, context.metadata, "positive"),
        ),
      )
    case "coffee_staking_claim_rewards":
      return sourceDestinationAction(
        action.details.pool,
        action.details.recipient,
        context.ownerAddress,
        isIncoming =>
          valueLines(
            assetValueLine(
              action.details.amount,
              action.details.asset,
              context.metadata,
              isIncoming ? "positive" : "negative",
            ),
          ),
        "Pool",
        {
          destinationAccounts: [action.details.recipient_jetton_wallet],
          sourceAccounts: [action.details.pool_jetton_wallet],
        },
      )
    case "coffee_mev_protect_hold_funds":
      return sourceDestinationAction(
        action.details.source,
        action.details.mev_contract,
        context.ownerAddress,
        () =>
          valueLines(
            assetValueLine(
              action.details.amount,
              action.details.asset,
              context.metadata,
              "negative",
            ),
          ),
        "MEV",
        {
          destinationAccounts: [action.details.mev_contract_jetton_wallet],
          sourceAccounts: [action.details.source_jetton_wallet],
        },
      )
    case "coffee_create_vault":
      return sourceDestinationAction(
        action.details.source,
        action.details.vault,
        context.ownerAddress,
        () => valueLines(tonValueLine(action.details.value, "negative")),
        "Vault",
      )
    case "layerzero_send":
      return sourceDestinationAction(
        action.details.initiator,
        action.details.layerzero_send_data.endpoint,
        context.ownerAddress,
        () => EMPTY_VALUE_LINES,
        "LayerZero",
      )
    case "layerzero_send_tokens":
      return sourceDestinationAction(
        action.details.sender,
        action.details.oapp,
        context.ownerAddress,
        () =>
          valueLines(
            assetValueLine(
              action.details.amount,
              action.details.asset,
              context.metadata,
              "negative",
            ),
          ),
        "LayerZero",
        {
          destinationAccounts: [action.details.oapp_wallet],
          sourceAccounts: [action.details.sender_wallet],
        },
      )
    case "layerzero_receive":
      return sourceDestinationAction(
        action.details.sender,
        action.details.oapp,
        context.ownerAddress,
        () => EMPTY_VALUE_LINES,
        "LayerZero",
      )
    case "layerzero_commit_packet":
      return sourceDestinationAction(
        action.details.sender,
        action.details.endpoint,
        context.ownerAddress,
        () => EMPTY_VALUE_LINES,
        "LayerZero",
      )
    case "layerzero_dvn_verify":
      return addressAction(action.details.uln, "uln", false, "LayerZero", EMPTY_VALUE_LINES)
    case "cocoon_worker_payout":
      return sourceDestinationAction(
        action.details.source,
        action.details.destination,
        context.ownerAddress,
        isIncoming =>
          valueLines(rawValueLine(action.details.amount, isIncoming ? "positive" : "negative")),
        "Cocoon",
      )
    case "cocoon_proxy_payout":
    case "cocoon_proxy_charge":
      return sourceDestinationAction(
        action.details.source,
        action.details.destination,
        context.ownerAddress,
        () => EMPTY_VALUE_LINES,
        "Cocoon",
      )
    case "cocoon_client_top_up":
    case "cocoon_grant_refund":
    case "cocoon_client_increase_stake":
    case "cocoon_client_withdraw":
      return sourceDestinationAction(
        action.details.source,
        action.details.destination,
        context.ownerAddress,
        isIncoming =>
          valueLines(rawValueLine(action.details.amount, isIncoming ? "positive" : "negative")),
        "Cocoon",
      )
    case "cocoon_register_proxy":
    case "cocoon_unregister_proxy":
      return addressAction(action.details.destination, "proxy", false, "Cocoon", EMPTY_VALUE_LINES)
    case "cocoon_client_register":
    case "cocoon_client_change_secret_hash":
    case "cocoon_client_request_refund":
      return sourceDestinationAction(
        action.details.source,
        action.details.destination,
        context.ownerAddress,
        () => EMPTY_VALUE_LINES,
        "Cocoon",
      )
    default:
      return unsupportedActionDisplay(action)
  }
}

function unsupportedActionDisplay(action: never): HistoryActionDisplay {
  void action
  return addressAction(null, undefined, false, "Action", EMPTY_VALUE_LINES)
}

function sourceDestinationAction(
  source: string | null,
  destination: string | null,
  ownerAddress: string,
  getValueLines: (isIncoming: boolean) => readonly HistoryValueLine[],
  displayAddressFallback: string,
  options: {
    readonly destinationAccounts?: readonly (string | null | undefined)[]
    readonly sourceAccounts?: readonly (string | null | undefined)[]
  } = {},
): HistoryActionDisplay {
  const isSourceAccount = matchesAnyActionAddress(
    [source, ...(options.sourceAccounts ?? [])],
    ownerAddress,
  )
  const isIncoming =
    !isSourceAccount &&
    matchesAnyActionAddress([destination, ...(options.destinationAccounts ?? [])], ownerAddress)
  return {
    isIncoming,
    address: isIncoming ? (source ?? "") : (destination ?? ""),
    displayAddressFallback,
    relationLabel: isIncoming ? "from" : "to",
    valueLines: getValueLines(isIncoming),
  }
}

function matchesAnyActionAddress(
  addresses: readonly (string | null | undefined)[],
  ownerAddress: string,
): boolean {
  return addresses.some(address => (address ? isSameAddress(address, ownerAddress) : false))
}

function addressAction(
  address: string | null,
  relationLabel: string | undefined,
  isIncoming: boolean,
  displayAddressFallback: string,
  valueLines: readonly HistoryValueLine[],
): HistoryActionDisplay {
  return {
    isIncoming,
    address: address ?? "",
    displayAddressFallback,
    relationLabel,
    valueLines,
  }
}

function valueLines(
  ...lines: readonly (HistoryValueLine | undefined)[]
): readonly HistoryValueLine[] {
  const compact = lines.filter((line): line is HistoryValueLine => line !== undefined)
  return compact.length > 0 ? compact.slice(0, 2) : EMPTY_VALUE_LINES
}

interface ValueLineOptions {
  readonly maximumFractionDigits?: number
  readonly showSign?: boolean
}

function tonValueLine(
  amount: string | null | undefined,
  tone: HistoryValueTone,
  options: ValueLineOptions = {},
): HistoryTextValueLine | undefined {
  if (!isNonEmptyString(amount)) {
    return undefined
  }

  const signlessAmount = amount.trim().replace(/^[+-]/, "")
  const readableAmount = formatNano(signlessAmount, options.maximumFractionDigits)
  const displayTone = isZeroDisplayNumber(readableAmount) ? "neutral" : tone
  const sign = options.showSign === false ? "" : valueSign(displayTone)
  return {
    kind: "text",
    label: `${sign}${readableAmount} GRAM`,
    tone: displayTone,
  }
}

function assetOrTonValueLine(
  amount: string | null | undefined,
  asset: string | null | undefined,
  metadata: V3Metadata,
  tone: HistoryValueTone,
): HistoryTextValueLine | undefined {
  return asset ? assetValueLine(amount, asset, metadata, tone) : tonValueLine(amount, tone)
}

function assetValueLine(
  amount: string | null | undefined,
  asset: string | null | undefined,
  metadata: V3Metadata,
  tone: HistoryValueTone,
  options: ValueLineOptions = {},
): HistoryTextValueLine | undefined {
  if (!isNonEmptyString(amount)) {
    return undefined
  }

  if (!isNonEmptyString(asset)) {
    return tonValueLine(amount, tone, options)
  }

  const tokenInfo = getMetadataTokenInfo(metadata, asset, "jetton_masters")
  const decimals = metadataTokenDecimals(tokenInfo)
  const symbol = metadataTokenString(tokenInfo, "symbol")
  const normalizedAmount = amount.trim().replace(/^[+-]/, "")
  const formattedAmount =
    decimals === undefined ? normalizedAmount : formatDecimalAmount(normalizedAmount, decimals)
  const readableAmount = formatReadableNumber(formattedAmount, options.maximumFractionDigits)
  const displayTone = isZeroDisplayNumber(readableAmount) ? "neutral" : tone
  const sign = options.showSign === false ? "" : valueSign(displayTone)
  return {
    kind: "text",
    label: `${sign}${readableAmount}${symbol ? ` ${symbol}` : ""}`,
    tone: displayTone,
  }
}

function swapValueLine(
  amountIn: string | null | undefined,
  assetIn: string | null | undefined,
  amountOut: string | null | undefined,
  assetOut: string | null | undefined,
  metadata: V3Metadata,
): HistorySwapValueLine | undefined {
  const options = {maximumFractionDigits: 3, showSign: false} as const
  const from = assetValueLine(amountIn, assetIn, metadata, "negative", options)
  const to = assetValueLine(amountOut, assetOut, metadata, "positive", options)
  return from && to ? {kind: "swap", from, to} : undefined
}

function rawValueLine(
  amount: string | null | undefined,
  tone: HistoryValueTone,
): HistoryTextValueLine | undefined {
  if (!isNonEmptyString(amount)) {
    return undefined
  }

  const normalizedAmount = amount.trim().replace(/^[+-]/, "")
  const readableAmount = formatReadableNumber(normalizedAmount)
  const displayTone = isZeroDisplayNumber(readableAmount) ? "neutral" : tone
  return {
    kind: "text",
    label: `${valueSign(displayTone)}${readableAmount}`,
    tone: displayTone,
  }
}

function textValueLine(
  value: string | null | undefined,
  tone: HistoryValueTone,
): HistoryTextValueLine | undefined {
  if (!isNonEmptyString(value)) {
    return undefined
  }

  return {kind: "text", label: value, tone}
}

function nftValueLine(
  itemAddress: string | null | undefined,
  itemIndex: string | null | undefined,
  metadata: V3Metadata,
): HistoryTextValueLine | undefined {
  const tokenInfo = itemAddress
    ? getMetadataTokenInfo(metadata, itemAddress, "nft_items")
    : undefined
  const name = metadataTokenString(tokenInfo, "name")
  if (name) {
    return {kind: "text", label: name, tone: "neutral"}
  }
  if (isNonEmptyString(itemIndex)) {
    return {kind: "text", label: `NFT #${itemIndex}`, tone: "neutral"}
  }
  if (itemAddress) {
    return {kind: "text", label: "NFT", tone: "neutral"}
  }
  return undefined
}

function getMetadataTokenInfo(
  metadata: V3Metadata,
  address: string,
  type: string,
): AccountStateTokenInfo | undefined {
  const entries = metadata[address]?.token_info ?? metadata[addressKey(address)]?.token_info ?? []
  return entries.find(info => info.type === type)
}

function metadataTokenString(
  tokenInfo: AccountStateTokenInfo | undefined,
  key: string,
): string | undefined {
  const value = tokenInfo?.[key]
  if (isNonEmptyString(value)) {
    return value
  }

  const extra = isRecord(tokenInfo?.extra) ? tokenInfo.extra : undefined
  const extraValue = extra?.[key]
  return isNonEmptyString(extraValue) ? extraValue : undefined
}

function metadataTokenDecimals(tokenInfo: AccountStateTokenInfo | undefined): number | undefined {
  const rawDecimals = metadataTokenString(tokenInfo, "decimals")
  if (!rawDecimals) {
    return undefined
  }

  const decimals = Number(rawDecimals)
  return Number.isInteger(decimals) && decimals >= 0 && decimals <= 36 ? decimals : undefined
}

function formatDecimalAmount(value: string, decimals: number): string {
  if (!/^[0-9]+$/.test(value)) {
    return value
  }

  try {
    const raw = BigInt(value)
    const divisor = 10n ** BigInt(decimals)
    const whole = raw / divisor
    const fraction = raw % divisor
    if (decimals === 0 || fraction === 0n) {
      return whole.toString()
    }

    const fractionText = fraction.toString().padStart(decimals, "0").replace(/0+$/, "")
    return `${whole}.${fractionText}`
  } catch {
    return value
  }
}

function formatReadableNumber(value: string, maximumFractionDigits = 9): string {
  const numeric = Number(value)
  return Number.isFinite(numeric) && value.length < 18
    ? numeric.toLocaleString(undefined, {maximumFractionDigits})
    : value
}

function valueSign(tone: HistoryValueTone): string {
  if (tone === "positive") {
    return "+ "
  }
  if (tone === "negative") {
    return "- "
  }
  return ""
}

function isZeroDisplayNumber(value: string): boolean {
  const numeric = Number(value.replaceAll(",", "").replaceAll(" ", ""))
  return Number.isFinite(numeric) && numeric === 0
}

function compareActionsByTime(left: HistoryActionInfo, right: HistoryActionInfo): number {
  if (left.utime !== right.utime) {
    return left.utime - right.utime
  }

  return left.rowKey.localeCompare(right.rowKey)
}

function historyValueToneClass(tone: HistoryValueTone): string {
  if (tone === "positive") {
    return styles.valuePositive
  }
  if (tone === "negative") {
    return styles.valueNegative
  }
  if (tone === "neutral") {
    return styles.valueNeutral
  }
  return styles.valueEmpty
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0
}

function compareTransactionsByTime(
  left: V3TransactionListItem,
  right: V3TransactionListItem,
): number {
  if (left.now !== right.now) {
    return left.now - right.now
  }

  const ltComparison = compareBigIntStrings(left.lt, right.lt)
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
  if (utime <= 0) {
    return {label: "-", title: "Unknown time"}
  }

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
  message: V3Message | undefined,
  messageNamesByAddress: MessageNamesByAddress,
): string | undefined {
  if (!message) {
    return undefined
  }

  const opcode = normalizeOpcode(message.opcode)
  if (!opcode) {
    return undefined
  }

  return resolveOpcodeName(opcode, message.source, message.destination, messageNamesByAddress)
}

function resolveOpcodeName(
  opcode: string,
  source: string | null | undefined,
  destination: string | null | undefined,
  messageNamesByAddress: MessageNamesByAddress,
): string | undefined {
  const destinationNames = destination
    ? messageNamesByAddress.get(addressKey(destination))
    : undefined
  const sourceNames = source ? messageNamesByAddress.get(addressKey(source)) : undefined

  return destinationNames?.incoming.get(opcode) ?? sourceNames?.outgoing.get(opcode) ?? undefined
}

function normalizeOpcode(opcode: string | number | null | undefined): string | undefined {
  if (opcode === null || opcode === undefined) {
    return undefined
  }

  const normalized = typeof opcode === "string" ? opcode.trim() : opcode
  if (normalized === "") {
    return undefined
  }

  try {
    const value =
      typeof normalized === "number"
        ? normalized
        : normalized.startsWith("0x") || normalized.startsWith("0X")
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

const ContractCode = lazy(async () => {
  const module = await import("./ContractCode")
  return {default: module.ContractCode}
})

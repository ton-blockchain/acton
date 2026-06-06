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
  Calendar,
  ChevronLeft,
  ChevronRight,
  Coins,
  Filter,
  Image,
  MessageSquare,
  MoreHorizontal,
  RefreshCw,
} from "lucide-react"
import type React from "react"
import {lazy, Suspense, useEffect, useMemo, useState} from "react"
import {useNavigate} from "react-router-dom"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import type {
  FullAccountState,
  JettonMaster,
  JettonWallet,
  NftItem,
  Message,
  Transaction,
} from "../api/types"
import {TonClient} from "../api/client"
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
type PaginationItem = number | "ellipsis-left" | "ellipsis-right"

export const AccountDetails: React.FC<AccountDetailsProps> = ({
  transactions,
  accountState,
  compilerAbi,
  compilerAbiLoading = false,
  compilerAbiError,
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
  const [compilerAbiByAddress, setCompilerAbiByAddress] = useState<
    Map<string, ContractABI | undefined>
  >(new Map())

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
              .catch((): Record<string, ContractABI | null> => ({}))
          : {}
      const abiByCodeHash = new Map<string, ContractABI | undefined>()
      for (const codeHash of codeHashes) {
        abiByCodeHash.set(codeHash, fetchedAbis[codeHash] ?? undefined)
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

  const totalPages = Math.max(1, Math.ceil(transactions.length / ITEMS_PER_PAGE))
  const safeCurrentPage = Math.min(currentPage, totalPages)
  const startIndex = (safeCurrentPage - 1) * ITEMS_PER_PAGE
  const paginatedTransactions = transactions.slice(startIndex, startIndex + ITEMS_PER_PAGE)
  const paginationItems = useMemo(
    () => getPaginationItems(safeCurrentPage, totalPages),
    [safeCurrentPage, totalPages],
  )

  useEffect(() => {
    setCurrentPage(1)
  }, [ownerAddress])

  useEffect(() => {
    setCurrentPage(page => Math.min(page, totalPages))
  }, [totalPages])

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

  return (
    <Card className={styles.tableCard}>
      <div className={styles.tabs}>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "history" ? styles.tabActive : ""}`}
          onClick={() => handleTabClick("history")}
        >
          <RefreshCw size={14} /> History
        </button>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "tokens" ? styles.tabActive : ""}`}
          onClick={() => handleTabClick("tokens")}
        >
          <Coins size={14} /> Tokens
        </button>
        {(showHoldersTab || jettonMaster) && (
          <button
            type="button"
            className={`${styles.tab} ${activeTab === "holders" ? styles.tabActive : ""}`}
            onClick={() => handleTabClick("holders")}
          >
            <Filter size={14} /> Holders
          </button>
        )}
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "nfts" ? styles.tabActive : ""}`}
          onClick={() => handleTabClick("nfts")}
        >
          <Image size={14} /> NFTs
        </button>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "contract" ? styles.tabActive : ""}`}
          onClick={() => handleTabClick("contract")}
        >
          <MessageSquare size={14} /> Contract
        </button>
        <div className={styles.flexSpacer} />
        {activeTab === "history" && (
          <>
            <div className={styles.tab}>
              <Calendar size={14} />
            </div>
            <div className={styles.tab}>
              <Filter size={14} /> Filters
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
              ) : paginatedTransactions.length === 0 ? (
                <TableRow className={styles.emptyRow}>
                  <TableCell colSpan={4} className={styles.emptyCell}>
                    <div className={styles.tableState}>No transactions found.</div>
                  </TableCell>
                </TableRow>
              ) : (
                paginatedTransactions.map(tx => {
                  const inMsg = tx.in_msg
                  const inMsgSrc = parseAddress(inMsg.source || "")
                  const inMsgDest = parseAddress(inMsg.destination || "")
                  const isInboundToAccount =
                    inMsgDest && browsedAddr ? inMsgDest.equals(browsedAddr) : false
                  const isIncoming =
                    isInboundToAccount &&
                    browsedAddr !== undefined &&
                    inMsgSrc !== undefined &&
                    (!inMsgSrc.equals(browsedAddr) || tx.out_msgs.length === 0)

                  const inValue = BigInt(tx.in_msg.value || "0")
                  const outValue = tx.out_msgs.reduce(
                    (acc, msg) => acc + BigInt(msg.value || "0"),
                    BigInt(0),
                  )

                  const displayValue = isIncoming ? inValue : outValue
                  const valueStr = formatNano(displayValue.toString())

                  const address = isIncoming
                    ? tx.in_msg.source || ""
                    : tx.out_msgs.find(m => m.destination)?.destination || ""

                  const displayAddressFallback = isIncoming ? "External" : "Contract"

                  const displayMessage = isIncoming
                    ? tx.in_msg
                    : tx.out_msgs.find(m => m.destination) ||
                      tx.out_msgs.find(m => m.opcode) ||
                      tx.out_msgs[0]
                  const displayOpcode =
                    resolveMessageName(displayMessage, messageNamesByAddress) ||
                    displayMessage?.opcode ||
                    undefined

                  const isAddressHovered =
                    hoveredAddress && address ? isSameAddress(address, hoveredAddress) : false

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
                        {formatTimeAgo(tx.utime, nowSeconds)}
                      </TableCell>
                      <TableCell className={styles.actionColumn}>
                        <div className={styles.action}>
                          {isIncoming ? (
                            <ArrowDownLeft
                              className={`${styles.actionIcon} ${styles.statusSuccess}`}
                            />
                          ) : (
                            <ArrowUpRight
                              className={`${styles.actionIcon} ${styles.statusFailed}`}
                            />
                          )}
                          {displayOpcode ? (
                            <span className={`${styles.actionText} ${styles.opcode}`}>
                              {displayOpcode}
                            </span>
                          ) : (
                            <span className={styles.actionText}>
                              {isIncoming ? "Received TON" : "Sent TON"}
                            </span>
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
                              if (address) onAddressClick?.(address)
                            }}
                            onMouseEnter={() => address && setHoveredAddress(address)}
                            onMouseLeave={() => setHoveredAddress(undefined)}
                          >
                            <AddressLabel address={address} fallback={displayAddressFallback} />
                          </button>
                        </div>
                      </TableCell>
                      <TableCell className={styles.valueContainer}>
                        <div
                          className={`${isIncoming ? styles.valuePositive : styles.valueNegative} ${styles.historyValue}`}
                        >
                          {isIncoming ? "+" : "-"} {Number.parseFloat(valueStr).toLocaleString()}{" "}
                          TON
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
                compilerAbi={compilerAbi}
                compilerAbiLoading={compilerAbiLoading}
                compilerAbiError={compilerAbiError}
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

function resolveMessageName(
  message: Message | undefined,
  messageNamesByAddress: Map<
    string,
    {incoming: Map<string, string>; outgoing: Map<string, string>}
  >,
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

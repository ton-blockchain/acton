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
  RefreshCw,
} from "lucide-react"
import type React from "react"
import {useMemo, useState, useEffect} from "react"
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
import {ContractCode} from "./ContractCode"
import {Nfts} from "./Nfts"
import {Tokens} from "./Tokens"
import styles from "./AccountDetails.module.css"
import {formatNano, formatTimeAgo, hashToHex, isSameAddress, parseAddress} from "./utils"

type Tabs = "history" | "contract" | "tokens" | "nfts" | "holders"

interface AccountDetailsProps {
  readonly transactions: Transaction[]
  readonly accountState: FullAccountState
  readonly accountCodeHash?: string
  readonly ownerAddress: string
  readonly jettonWallets: JettonWallet[]
  readonly nftItems: NftItem[]
  readonly jettonMaster?: JettonMaster
  readonly holders?: JettonWallet[]
  readonly client: TonClient
  readonly onAddressClick?: (addr: string) => void
  readonly activeTabHash?: string
  readonly onTabChange?: (tab: Tabs) => void
}

const ITEMS_PER_PAGE = 10

export const AccountDetails: React.FC<AccountDetailsProps> = ({
  transactions,
  accountState,
  accountCodeHash,
  ownerAddress,
  jettonWallets,
  nftItems,
  jettonMaster,
  holders,
  client,
  onAddressClick,
  activeTabHash,
  onTabChange,
}) => {
  const navigate = useNavigate()
  const [activeTab, setActiveTab] = useState<Tabs>("history")
  const [compilerAbi, setCompilerAbi] = useState<ContractABI | null | undefined>()
  const [compilerAbiError, setCompilerAbiError] = useState<string | undefined>()
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

    const loadCompilerAbi = async () => {
      if (!accountCodeHash) {
        setCompilerAbi(undefined)
        setCompilerAbiError(undefined)
        return
      }

      setCompilerAbi(undefined)
      setCompilerAbiError(undefined)

      try {
        const abi = await client.getCompilerAbi(accountCodeHash)
        if (!isActive) return
        setCompilerAbi(abi)
      } catch (error) {
        if (!isActive) return
        setCompilerAbi(undefined)
        setCompilerAbiError(error instanceof Error ? error.message : "Failed to load compiler ABI")
      }
    }

    void loadCompilerAbi()
    return () => {
      isActive = false
    }
  }, [accountCodeHash, client])

  useEffect(() => {
    let isActive = true

    const loadRelatedCompilerAbis = async () => {
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
      if (compilerAbi !== undefined) {
        next.set(ownerKey, compilerAbi ?? undefined)
      }

      const states = await client.getAccountStates(requestedAddresses, false).catch(() => {})
      const addressToCodeHash = new Map<string, string>()
      for (const account of states?.accounts ?? []) {
        if (account.code_hash) {
          addressToCodeHash.set(addressKey(account.address), account.code_hash)
        }
      }

      const codeHashesToFetch = new Set<string>()
      for (const [address, codeHash] of addressToCodeHash) {
        if (
          address === ownerKey &&
          accountCodeHash &&
          compilerAbi !== undefined &&
          codeHash === accountCodeHash
        ) {
          continue
        }
        codeHashesToFetch.add(codeHash)
      }

      const fetchedAbis = await Promise.all(
        [...codeHashesToFetch].map(async codeHash => {
          try {
            return [codeHash, await client.getCompilerAbi(codeHash)] as const
          } catch {
            return [codeHash, undefined] as const
          }
        }),
      )
      const abiByCodeHash = new Map<string, ContractABI | undefined>(fetchedAbis)
      if (accountCodeHash && compilerAbi !== undefined) {
        abiByCodeHash.set(accountCodeHash, compilerAbi ?? undefined)
      }

      for (const address of requestedAddresses) {
        const key = addressKey(address)
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
  }, [transactions, ownerAddress, accountCodeHash, compilerAbi, client])

  const handleTabClick = (tab: Tabs) => {
    setActiveTab(tab)
    onTabChange?.(tab)
  }

  const [currentPage, setCurrentPage] = useState(1)
  const [hoveredAddress, setHoveredAddress] = useState<string | undefined>()

  const totalPages = Math.ceil(transactions.length / ITEMS_PER_PAGE)
  const startIndex = (currentPage - 1) * ITEMS_PER_PAGE
  const paginatedTransactions = transactions.slice(startIndex, startIndex + ITEMS_PER_PAGE)

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
        {jettonMaster && (
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
            <TableHeader>
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
              {paginatedTransactions.map(tx => {
                const inMsg = tx.in_msg
                const inMsgSrc = parseAddress(inMsg.source || "")
                const isIncoming = inMsgSrc && browsedAddr ? !inMsgSrc.equals(browsedAddr) : false

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
                      {formatTimeAgo(tx.utime)}
                    </TableCell>
                    <TableCell className={styles.actionColumn}>
                      <div className={styles.action}>
                        {isIncoming ? (
                          <ArrowDownLeft
                            className={`${styles.actionIcon} ${styles.statusSuccess}`}
                          />
                        ) : (
                          <ArrowUpRight className={`${styles.actionIcon} ${styles.statusFailed}`} />
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
                      <div className={isIncoming ? styles.valuePositive : styles.valueNegative}>
                        {isIncoming ? "+" : "-"} {Number.parseFloat(valueStr).toLocaleString()} TON
                      </div>
                    </TableCell>
                  </TableRow>
                )
              })}
            </TableBody>
          </Table>

          {totalPages > 1 && (
            <div className={styles.pagination}>
              <button
                type="button"
                className={styles.paginationButton}
                onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                disabled={currentPage === 1}
              >
                <ChevronLeft size={16} />
              </button>
              <span className={styles.paginationInfo}>
                Page {currentPage} of {totalPages}
              </span>
              <button
                type="button"
                className={styles.paginationButton}
                onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
                disabled={currentPage === totalPages}
              >
                <ChevronRight size={16} />
              </button>
            </div>
          )}
        </CardContent>
      ) : activeTab === "tokens" ? (
        <CardContent className={styles.tokensContent}>
          <Tokens wallets={jettonWallets} client={client} onAddressClick={onAddressClick} />
        </CardContent>
      ) : activeTab === "nfts" ? (
        <CardContent className={styles.tokensContent}>
          <Nfts items={nftItems} onAddressClick={onAddressClick} />
        </CardContent>
      ) : activeTab === "holders" ? (
        <CardContent className={styles.historyContent}>
          <Table>
            <TableHeader>
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
                  <TableRow key={holder.address} className={styles.row}>
                    <TableCell>
                      <button
                        type="button"
                        className={styles.address}
                        onClick={() => onAddressClick?.(holder.owner)}
                      >
                        <AddressLabel address={holder.owner} />
                      </button>
                    </TableCell>
                    <TableCell>
                      <button
                        type="button"
                        className={styles.address}
                        onClick={() => onAddressClick?.(holder.address)}
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
            </TableBody>
          </Table>
          {(!holders || holders.length === 0) && (
            <div className={styles.empty}>No holders found.</div>
          )}
        </CardContent>
      ) : (
        <ContractCode
          codeBoc={accountState.code}
          compilerAbi={compilerAbi}
          compilerAbiLoading={compilerAbi === undefined}
          compilerAbiError={compilerAbiError}
        />
      )}
    </Card>
  )
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

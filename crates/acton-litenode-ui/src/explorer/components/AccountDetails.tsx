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
  MessageSquare,
  RefreshCw,
} from "lucide-react"
import type React from "react"
import {useMemo, useState, useEffect} from "react"
import {useNavigate} from "react-router-dom"

import type {FullAccountState, JettonMaster, JettonWallet, Transaction} from "../api/types"
import {TonClient} from "../api/client"

import {AddressLabel} from "./AddressLabel"
import {ContractCode} from "./ContractCode"
import {Tokens} from "./Tokens"
import styles from "./AccountDetails.module.css"
import {formatNano, formatTimeAgo, isSameAddress, parseAddress} from "./utils"

type Tabs = "history" | "contract" | "tokens" | "holders"

interface AccountDetailsProps {
  readonly transactions: Transaction[]
  readonly accountState: FullAccountState
  readonly ownerAddress: string
  readonly jettonWallets: JettonWallet[]
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
  ownerAddress,
  jettonWallets,
  jettonMaster,
  holders,
  client,
  onAddressClick,
  activeTabHash,
  onTabChange,
}) => {
  const navigate = useNavigate()
  const [activeTab, setActiveTab] = useState<Tabs>("history")

  useEffect(() => {
    if (
      activeTabHash &&
      (activeTabHash === "history" ||
        activeTabHash === "contract" ||
        activeTabHash === "tokens" ||
        activeTabHash === "holders")
    ) {
      setActiveTab(activeTabHash as Tabs)
    }
  }, [activeTabHash])

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
        <div className={styles.tab}>
          <div className={styles.nftIcon} /> NFTs
        </div>
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
                <TableHead className={styles.tableHeader}>Time</TableHead>
                <TableHead className={styles.tableHeader}>Action</TableHead>
                <TableHead className={styles.tableHeader}>Address</TableHead>
                <TableHead className={`${styles.tableHeader} ${styles.valueContainer}`}>
                  Value
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {paginatedTransactions.map(tx => {
                const inMsg = tx.in_msg
                const inMsgSrc = parseAddress(inMsg.source?.account_address || "")
                const isIncoming = inMsgSrc && browsedAddr ? !inMsgSrc.equals(browsedAddr) : false

                const inValue = BigInt(tx.in_msg.value || "0")
                const outValue = tx.out_msgs.reduce(
                  (acc, msg) => acc + BigInt(msg.value || "0"),
                  BigInt(0),
                )

                const displayValue = isIncoming ? inValue : outValue
                const valueStr = formatNano(displayValue.toString())

                const address = isIncoming
                  ? tx.in_msg.source?.account_address || ""
                  : tx.out_msgs.find(m => m.destination?.account_address)?.destination
                      ?.account_address || ""

                const displayAddressFallback = isIncoming ? "External" : "Contract"

                const opcode = isIncoming
                  ? tx.in_msg.opcode
                  : tx.out_msgs.find(m => m.opcode)?.opcode
                const displayOpcode = opcode ?? undefined

                const isAddressHovered =
                  hoveredAddress && address ? isSameAddress(address, hoveredAddress) : false

                return (
                  <TableRow
                    key={tx.hash}
                    className={`${styles.row} ${styles.clickableRow}`}
                    onClick={() => {
                      void navigate(`/tx/${tx.hash}`)
                    }}
                  >
                    <TableCell className={styles.time}>{formatTimeAgo(tx.utime)}</TableCell>
                    <TableCell>
                      <div className={styles.action}>
                        {isIncoming ? (
                          <ArrowDownLeft
                            className={`${styles.actionIcon} ${styles.statusSuccess}`}
                          />
                        ) : (
                          <ArrowUpRight className={`${styles.actionIcon} ${styles.statusFailed}`} />
                        )}
                        <span>{isIncoming ? "Received TON" : "Sent TON"}</span>
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
                        {displayOpcode && <span className={styles.opcode}>{displayOpcode}</span>}
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
        <ContractCode codeBoc={accountState.code} />
      )}
    </Card>
  )
}

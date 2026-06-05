import {
  type ContractData,
  TransactionDetails,
  type TransactionInfo,
  TransactionTree,
  ValueFlowTable,
  type ValueFlowItem,
} from "@acton/shared-ui"
import {Address} from "@ton/core"
import {Activity, AlertCircle, ArrowLeft, CheckCircle2, List, Loader2, XCircle} from "lucide-react"
import type React from "react"
import {useEffect, useLayoutEffect, useMemo, useRef, useState} from "react"
import {useNavigate, useParams, useSearchParams} from "react-router-dom"

import type {TonClient} from "../api/client"
import {buildTraceTransactionInfos} from "../api/traceTransactions"
import type {V3Transaction} from "../api/types"
import {addressKey} from "../api/compilerAbi"
import {Breadcrumbs} from "../components/Breadcrumbs"
import {
  formatAddress as formatDisplayAddress,
  hashToHex,
  normalizeAddress,
} from "../components/utils"
import {useAddressBook} from "../hooks/useAddressBook"
import {useAddressFormat} from "../hooks/useNetworkInfo"

import styles from "./TransactionPage.module.css"

interface TransactionPageProps {
  readonly client: TonClient
}

type TabType = "transactions" | "value-flow"

const parseTabType = (tab: string | null): TabType => {
  return tab === "transactions" ? "transactions" : "value-flow"
}

interface ValueFlowAccumulator extends ValueFlowItem {
  readonly before: bigint
  readonly after: bigint
}

interface TraceTransactionNodeProps {
  readonly tx: TransactionInfo
  readonly contracts: Map<string, ContractData>
  readonly isIntermediateSibling?: boolean
  readonly onContractClick: (address: string) => void
}

const buildTransactionsHexIndex = (
  transactionsMap: Record<string, V3Transaction>,
): Record<string, V3Transaction> => {
  const indexed: Record<string, V3Transaction> = {}

  for (const [mapKey, tx] of Object.entries(transactionsMap)) {
    const normalizedHash = hashToHex(mapKey) ?? hashToHex(tx.hash) ?? mapKey
    indexed[normalizedHash.toLowerCase()] = tx
  }

  return indexed
}

export const TransactionPage: React.FC<TransactionPageProps> = ({client}) => {
  const {hash: routeHash = ""} = useParams<{hash: string}>()
  const hash = hashToHex(routeHash) ?? routeHash
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  const [loading, setLoading] = useState(true)
  const [traces, setTraces] = useState<TransactionInfo[]>([])
  const [contracts, setContracts] = useState<Map<string, ContractData>>(new Map())
  const [error, setError] = useState<string | undefined>()
  const [activeTab, setActiveTab] = useState<TabType>(() => parseTabType(searchParams.get("tab")))
  const [valueFlow, setValueFlow] = useState<ValueFlowItem[]>([])
  const {fetchName} = useAddressBook()
  const addressFormat = useAddressFormat()

  const handleContractClick = (address: string) => {
    const formattedAddr = normalizeAddress(address, addressFormat)
    void navigate(`/explorer/address/${encodeURIComponent(formattedAddr)}`)
  }

  const handleActiveTabChange = (tab: TabType) => {
    setActiveTab(tab)
    setSearchParams(
      currentSearchParams => {
        const nextSearchParams = new URLSearchParams(currentSearchParams)
        nextSearchParams.set("tab", tab)
        return nextSearchParams
      },
      {replace: true},
    )
  }

  useEffect(() => {
    setActiveTab(parseTabType(searchParams.get("tab")))
  }, [searchParams])

  useEffect(() => {
    if (!hash) return
    let isActive = true

    const fetchTrace = async () => {
      setLoading(true)
      setError(undefined)
      try {
        const data = await client.getTraces(hash)

        if (data.traces && data.traces.length > 0) {
          const trace = data.traces[0]
          const transactionsMap = trace.transactions
          const transactionsByHex = buildTransactionsHexIndex(transactionsMap)

          const processed = buildTraceTransactionInfos(transactionsMap, trace.trace)
          if (!isActive) return
          setTraces(processed)

          const contractsMap = new Map<string, ContractData>()
          const addresses = new Set<string>()

          for (const t of processed) {
            if (t.address) addresses.add(t.address.toString())
          }

          const requestedAddresses = [...addresses].sort()
          const states =
            requestedAddresses.length > 0
              ? await client.getAccountStates(requestedAddresses, false).catch(() => {})
              : undefined
          const addressToCodeHash = new Map<string, string>()
          for (const account of states?.accounts ?? []) {
            if (account.code_hash) {
              addressToCodeHash.set(addressKey(account.address), account.code_hash)
            }
          }

          const abiByCodeHash = new Map<string, ContractData["abi"]>()
          const codeHashes = [...new Set(addressToCodeHash.values())]
          const fetchedAbis =
            codeHashes.length > 0
              ? await client
                  .getCompilerAbis(codeHashes)
                  .catch((): Record<string, ContractData["abi"] | null> => ({}))
              : {}
          for (const codeHash of codeHashes) {
            abiByCodeHash.set(codeHash, fetchedAbis[codeHash] ?? undefined)
          }

          let nextLetterCode = 65
          await Promise.all(
            requestedAddresses.map(async addr => {
              const letter = String.fromCodePoint(nextLetterCode++)
              const displayAddr = normalizeAddress(addr, addressFormat)
              const customName = await fetchName(addr)
              const abi = abiByCodeHash.get(addressToCodeHash.get(addressKey(addr)) ?? "")
              contractsMap.set(addr, {
                displayName: customName || formatDisplayAddress(displayAddr, true, addressFormat),
                address: Address.parse(addr),
                letter,
                abi,
              })
            }),
          )
          if (!isActive) return
          setContracts(contractsMap)

          setValueFlow(buildValueFlowItems(transactionsByHex, processed))
        } else {
          if (isActive) setError("Transaction not found or has no trace yet.")
        }
      } catch (error) {
        console.error("Failed to fetch trace:", error)
        if (!isActive) return
        setError(error instanceof Error ? error.message : "Failed to load transaction trace")
      } finally {
        if (isActive) setLoading(false)
      }
    }

    void fetchTrace()
    return () => {
      isActive = false
    }
  }, [addressFormat, client, fetchName, hash])

  if (loading) {
    return (
      <div className={styles.centered}>
        <Loader2 className={styles.spinner} />
        <p>Loading transaction trace...</p>
      </div>
    )
  }

  if (error) {
    return (
      <div className={styles.centered}>
        <AlertCircle className={styles.errorIcon} />
        <p className={styles.errorText}>{error}</p>
        <button type="button" onClick={() => void navigate(-1)} className={styles.backButton}>
          <ArrowLeft size={16} /> Go Back
        </button>
      </div>
    )
  }

  const firstTrace = traces[0]
  const traceAddress = firstTrace?.address?.toString() ?? ""
  const traceAddressDisplay = normalizeAddress(traceAddress, addressFormat)
  const rootTraceTransactions = [...traces]
    .filter(tx => !tx.parent)
    .sort(compareTransactionInfoByLt)

  return (
    <div className={styles.container}>
      <div className={styles.content}>
        {traces.length > 0 && (
          <>
            <Breadcrumbs
              items={[
                {
                  label: traceAddressDisplay,
                  path: `/explorer/address/${traceAddressDisplay}`,
                  isAddress: true,
                },
                {label: hash, isHash: true},
              ]}
            />
            <div className={styles.preTreeContent}>
              <div className={styles.overviewCard}>
                <div className={styles.overviewHeader}>
                  <div
                    className={`${styles.status} ${firstTrace.transaction.description.type === "generic" && firstTrace.transaction.description.computePhase.type === "vm" && firstTrace.transaction.description.computePhase.success ? styles.statusSuccess : styles.statusError}`}
                  >
                    {firstTrace.transaction.description.type === "generic" &&
                    firstTrace.transaction.description.computePhase.type === "vm" &&
                    firstTrace.transaction.description.computePhase.success ? (
                      <>
                        <CheckCircle2 size={18} /> Confirmed transaction
                      </>
                    ) : (
                      <>
                        <XCircle size={18} /> Failed transaction
                      </>
                    )}
                  </div>
                  <div className={styles.value}>
                    {new Date(firstTrace.transaction.now * 1000).toLocaleString()}
                  </div>
                </div>
              </div>

              <div className={styles.tabsContainer}>
                <div className={styles.tabs}>
                  <button
                    type="button"
                    className={`${styles.tab} ${activeTab === "value-flow" ? styles.tabActive : ""}`}
                    onClick={() => handleActiveTabChange("value-flow")}
                  >
                    <Activity size={16} /> Value Flow
                  </button>
                  <button
                    type="button"
                    className={`${styles.tab} ${activeTab === "transactions" ? styles.tabActive : ""}`}
                    onClick={() => handleActiveTabChange("transactions")}
                  >
                    <List size={16} /> Transactions
                  </button>
                </div>

                <div className={styles.tabContent}>
                  {activeTab === "value-flow" && (
                    <ValueFlowTable
                      items={valueFlow}
                      contracts={contracts}
                      onContractClick={handleContractClick}
                    />
                  )}

                  {activeTab === "transactions" && (
                    <div className={styles.detailsList}>
                      {rootTraceTransactions.map(tx => (
                        <TraceTransactionNode
                          key={tx.lt}
                          tx={tx}
                          contracts={contracts}
                          onContractClick={handleContractClick}
                        />
                      ))}
                    </div>
                  )}
                </div>
              </div>
            </div>

            <div className={styles.treeSection}>
              <TransactionTree
                transactions={traces}
                contracts={contracts}
                allContracts={[]}
                onContractClick={handleContractClick}
              />
            </div>
          </>
        )}
      </div>
    </div>
  )
}

const TraceTransactionNode: React.FC<TraceTransactionNodeProps> = ({
  tx,
  contracts,
  isIntermediateSibling = false,
  onContractClick,
}) => {
  const cardRef = useRef<HTMLDivElement>(null)
  const childrenRef = useRef<HTMLDivElement>(null)
  const [connectorHeight, setConnectorHeight] = useState(24)
  const children = useMemo(() => [...tx.children].sort(compareTransactionInfoByLt), [tx.children])

  useLayoutEffect(() => {
    if (children.length === 0) {
      return
    }

    let animationFrame = 0

    const updateConnectorHeight = () => {
      cancelAnimationFrame(animationFrame)
      animationFrame = requestAnimationFrame(() => {
        const card = cardRef.current
        const childrenContainer = childrenRef.current
        const lastChildCard = childrenContainer?.querySelector<HTMLElement>(
          `:scope > .${styles.traceNode}:last-child > .${styles.traceTransaction}`,
        )

        if (!card || !lastChildCard) {
          return
        }

        const cardRect = card.getBoundingClientRect()
        const lastChildRect = lastChildCard.getBoundingClientRect()
        const nextConnectorHeight = Math.max(
          24,
          Math.round(lastChildRect.top + 12 - cardRect.bottom),
        )

        setConnectorHeight(currentHeight =>
          currentHeight === nextConnectorHeight ? currentHeight : nextConnectorHeight,
        )
      })
    }

    updateConnectorHeight()

    const resizeObserver =
      typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(updateConnectorHeight)
    if (resizeObserver) {
      if (cardRef.current) {
        resizeObserver.observe(cardRef.current)
      }
      if (childrenRef.current) {
        resizeObserver.observe(childrenRef.current)
      }
    }

    window.addEventListener("resize", updateConnectorHeight)

    return () => {
      cancelAnimationFrame(animationFrame)
      resizeObserver?.disconnect()
      window.removeEventListener("resize", updateConnectorHeight)
    }
  }, [children.length])

  return (
    <div className={styles.traceNode}>
      <div ref={cardRef} className={styles.traceTransaction}>
        {isIntermediateSibling && <div className={styles.traceSiblingCurve} aria-hidden="true" />}
        <div className={styles.detailCard}>
          <TransactionDetails
            tx={tx}
            contracts={contracts}
            allContracts={[]}
            onContractClick={onContractClick}
          />
        </div>
        {children.length > 0 && (
          <div
            className={styles.traceConnectorAnchor}
            style={
              {
                "--trace-connector-height": `${connectorHeight}px`,
              } as React.CSSProperties
            }
            aria-hidden="true"
          >
            <div className={styles.traceConnectorRail} />
            <div className={styles.traceTerminalCurve} />
          </div>
        )}
      </div>
      {children.length > 0 && (
        <div ref={childrenRef} className={styles.traceChildren}>
          {children.map((child, index) => (
            <TraceTransactionNode
              key={child.lt}
              tx={child}
              contracts={contracts}
              isIntermediateSibling={index < children.length - 1}
              onContractClick={onContractClick}
            />
          ))}
        </div>
      )}
    </div>
  )
}

function buildValueFlowItems(
  transactionsByHex: Readonly<Record<string, V3Transaction>>,
  processed: readonly TransactionInfo[],
): ValueFlowItem[] {
  const flowByAddress = new Map<string, ValueFlowAccumulator>()

  for (const item of [...processed].sort(compareTransactionInfoByLt)) {
    const address = item.address?.toString()
    if (!address) {
      continue
    }

    const txHash = item.transaction.hash().toString("hex")
    const tx = transactionsByHex[txHash]
    if (!tx) {
      continue
    }

    const before = parseBalance(tx.account_state_before?.balance)
    const after = parseBalance(tx.account_state_after?.balance)
    if (before === undefined || after === undefined) {
      continue
    }

    const initialBefore = flowByAddress.get(address)?.before ?? before

    flowByAddress.set(address, {
      address,
      before: initialBefore,
      after,
      change: after - initialBefore,
      fee: (flowByAddress.get(address)?.fee ?? 0n) + item.transaction.totalFees.coins,
    })
  }

  return [...flowByAddress.values()]
    .map(({address, change, fee}) => ({address, change, fee}))
    .sort((a, b) => a.address.localeCompare(b.address))
}

function compareTransactionInfoByLt(left: TransactionInfo, right: TransactionInfo): number {
  const leftLt = parseBigInt(left.lt)
  const rightLt = parseBigInt(right.lt)
  if (leftLt === rightLt) {
    return 0
  }
  return leftLt < rightLt ? -1 : 1
}

function parseBalance(value: string | undefined): bigint | undefined {
  if (!value) {
    return undefined
  }

  try {
    return BigInt(value)
  } catch {
    return undefined
  }
}

function parseBigInt(value: string | undefined): bigint {
  try {
    return value === undefined ? 0n : BigInt(value)
  } catch {
    return 0n
  }
}

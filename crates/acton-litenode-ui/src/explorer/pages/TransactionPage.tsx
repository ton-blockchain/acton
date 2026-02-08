import {
  type BackendTransaction,
  ContractChip,
  type ContractData,
  fmt,
  processTransactions,
  TransactionDetails,
  type TransactionInfo,
  TransactionTree,
} from "@acton/shared-ui"
import {
  Activity,
  AlertCircle,
  ArrowLeft,
  CheckCircle2,
  List,
  Loader2,
  TrendingDown,
  TrendingUp,
  XCircle,
} from "lucide-react"
import type React from "react"
import { useEffect, useState } from "react"
import { useNavigate, useParams } from "react-router-dom"
import type { TonClient } from "../api/client"
import { Breadcrumbs } from "../components/Breadcrumbs"
import { fetchAddressName, normalizeAddress } from "../components/utils"
import styles from "./TransactionPage.module.css"
import {Address} from "@ton/core";

interface TransactionPageProps {
  client: TonClient
}

type TabType = "transactions" | "value-flow"

interface ValueFlowItem {
  address: string
  before: bigint
  after: bigint
  change: bigint
  fee: bigint
}

interface V3Transaction {
  hash: string
  lt: string
  raw_transaction?: string
  child_transactions?: string[]
  mc_block_seqno?: number
  [key: string]: unknown
}

interface V3Trace {
  transactions: Record<string, V3Transaction>
  [key: string]: unknown
}

interface V3TracesResponse {
  traces: V3Trace[]
}

const buildBackendTransactions = (
  transactionsMap: Record<string, V3Transaction>,
): BackendTransaction[] => {
  const findParentLt = (targetLt: string): string | null => {
    for (const tx of Object.values(transactionsMap)) {
      if (tx.child_transactions?.includes(targetLt)) {
        return tx.lt
      }
    }
    return null
  }

  return Object.values(transactionsMap).map((tx) => ({
    lt: tx.lt,
    raw_transaction: tx.raw_transaction || "",
    parent_transaction: findParentLt(tx.lt),
    child_transactions: tx.child_transactions || [],
    shard_account_before: "",
    shard_account: "",
    vm_log_diff: "",
    executor_logs: "",
  }))
}

const collectSeqnoBounds = (
  processed: TransactionInfo[],
  transactionsMap: Record<string, V3Transaction>,
) => {
  let minSeqno = Number.MAX_SAFE_INTEGER
  let maxSeqno = 0

  processed.forEach((t) => {
    const txHash = t.transaction.hash().toString("hex")
    const v3Tx = transactionsMap[txHash]
    const seqno = v3Tx?.mc_block_seqno || 0

    if (seqno > 0) {
      minSeqno = Math.min(minSeqno, seqno)
      maxSeqno = Math.max(maxSeqno, seqno)
    }
  })

  return { minSeqno, maxSeqno }
}

export const TransactionPage: React.FC<TransactionPageProps> = ({ client }) => {
  const { hash } = useParams<{ hash: string }>()
  const navigate = useNavigate()
  const [loading, setLoading] = useState(true)
  const [traces, setTraces] = useState<TransactionInfo[]>([])
  const [contracts, setContracts] = useState<Map<string, ContractData>>(new Map())
  const [error, setError] = useState<string | null>(null)
  const [activeTab, setActiveTab] = useState<TabType>("value-flow")
  const [valueFlow, setValueFlow] = useState<ValueFlowItem[]>([])
  const [loadingFlow, setLoadingFlow] = useState(false)

  const handleContractClick = (address: string) => {
    const formattedAddr = normalizeAddress(address)
    window.open(`/explorer/address/${formattedAddr}`)
  }

  useEffect(() => {
    if (!hash) return
    let isActive = true

    const fetchTrace = async () => {
      setLoading(true)
      setError(null)
      try {
        const data = (await client.getTraces(hash)) as V3TracesResponse

        if (data.traces && data.traces.length > 0) {
          const trace = data.traces[0]
          const transactionsMap = trace.transactions

          const backendTransactions = buildBackendTransactions(transactionsMap)

          const processed = processTransactions(backendTransactions)
          if (!isActive) return
          setTraces(processed)

          const contractsMap = new Map<string, ContractData>()
          const addresses = new Set<string>()

          processed.forEach((t) => {
            if (t.address) addresses.add(t.address.toString())
          })
          const { minSeqno, maxSeqno } = collectSeqnoBounds(processed, transactionsMap)

          let nextLetterCode = 65
          await Promise.all(
            Array.from(addresses)
              .sort()
              .map(async (addr) => {
                const displayAddr = normalizeAddress(addr)
                const customName = await fetchAddressName(addr)
                contractsMap.set(addr, {
                  displayName: customName || fmt.formatAddress(displayAddr),
                  address: Address.parse(addr),
                  letter: String.fromCharCode(nextLetterCode++),
                })
              }),
          )
          if (!isActive) return
          setContracts(contractsMap)

          // Fetch Value Flow
          if (addresses.size > 0 && minSeqno !== Number.MAX_SAFE_INTEGER) {
            setLoadingFlow(true)
            const flowItems: ValueFlowItem[] = []
            const uniqueAddrs = Array.from(addresses)

            await Promise.all(
              uniqueAddrs.map(async (addr) => {
                try {
                  // We fetch state before the trace (minSeqno - 1) and after (maxSeqno)
                  const [beforeState, afterState] = await Promise.all([
                    client.getAddressInformation(addr, minSeqno - 1),
                    client.getAddressInformation(addr, maxSeqno),
                  ])

                  const before = BigInt(beforeState.balance)
                  const after = BigInt(afterState.balance)

                  // Calculate total fees paid by this account in this trace
                  const accountFees = processed
                    .filter((t) => t.address?.toString() === addr)
                    .reduce((acc, t) => acc + t.transaction.totalFees.coins, 0n)

                  flowItems.push({
                    address: addr,
                    before,
                    after,
                    change: after - before,
                    fee: accountFees,
                  })
                } catch (e) {
                  console.warn(`Failed to fetch flow for ${addr}:`, e)
                }
              }),
            )

            if (!isActive) return
            setValueFlow(flowItems.sort((a, b) => a.address.localeCompare(b.address)))
            setLoadingFlow(false)
          }
        } else {
          if (isActive) setError("Transaction not found or has no trace yet.")
        }
      } catch (e) {
        console.error("Failed to fetch trace:", e)
        if (!isActive) return
        setError(e instanceof Error ? e.message : "Failed to load transaction trace")
      } finally {
        if (isActive) setLoading(false)
      }
    }

    void fetchTrace()
    return () => {
      isActive = false
    }
  }, [hash, client])

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
        <button type="button" onClick={() => navigate(-1)} className={styles.backButton}>
          <ArrowLeft size={16} /> Go Back
        </button>
      </div>
    )
  }

  const traceAddress = traces[0]?.address?.toString() ?? ""
  const traceAddressDisplay = normalizeAddress(traceAddress)

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
                { label: hash || "", isHash: true },
              ]}
            />
            <div className={styles.overviewCard}>
              <div className={styles.overviewHeader}>
                <div
                  className={`${styles.status} ${traces[0].transaction.description.type === "generic" && traces[0].transaction.description.computePhase.type === "vm" && traces[0].transaction.description.computePhase.success ? styles.statusSuccess : styles.statusError}`}
                >
                  {traces[0].transaction.description.type === "generic" &&
                  traces[0].transaction.description.computePhase.type === "vm" &&
                  traces[0].transaction.description.computePhase.success ? (
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
                  {new Date(traces[0].transaction.now * 1000).toLocaleString()}
                </div>
              </div>
            </div>

            <div className={styles.tabsContainer}>
              <div className={styles.tabs}>
                <button
                  type="button"
                  className={`${styles.tab} ${activeTab === "value-flow" ? styles.tabActive : ""}`}
                  onClick={() => setActiveTab("value-flow")}
                >
                  <Activity size={16} /> Value Flow
                </button>
                <button
                  type="button"
                  className={`${styles.tab} ${activeTab === "transactions" ? styles.tabActive : ""}`}
                  onClick={() => setActiveTab("transactions")}
                >
                  <List size={16} /> Transactions
                </button>
              </div>

              <div className={styles.tabContent}>
                {activeTab === "value-flow" && (
                  <div className={styles.valueFlowContainer}>
                    {loadingFlow ? (
                      <div className={styles.centered}>
                        <Loader2 className={styles.spinner} />
                        <p>Calculating value flow...</p>
                      </div>
                    ) : (
                      <div className={styles.flowList}>
                        <div className={styles.flowHeader}>
                          <div className={styles.flowCol}>Account</div>
                          <div className={styles.flowCol}>Balance Change</div>
                          <div className={styles.flowCol}>Network Fee</div>
                        </div>
                        {valueFlow.map((item) => (
                          <div key={item.address} className={styles.flowRow}>
                            <div className={styles.flowCol}>
                              <ContractChip
                                address={item.address}
                                contracts={contracts}
                                onContractClick={handleContractClick}
                              />
                            </div>
                            <div
                              className={`${styles.flowCol} ${item.change > 0n ? styles.statusSuccess : item.change < 0n ? styles.statusError : ""}`}
                            >
                              <div className={styles.changeValue}>
                                {item.change > 0n ? (
                                  <TrendingUp size={14} />
                                ) : item.change < 0n ? (
                                  <TrendingDown size={14} />
                                ) : null}
                                {fmt.formatCurrency(item.change)}
                              </div>
                            </div>
                            <div className={styles.flowCol}>{fmt.formatCurrency(item.fee)}</div>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                )}

                {activeTab === "transactions" && (
                  <div className={styles.detailsList}>
                    {traces
                      .sort((a, b) => Number(BigInt(a.lt) - BigInt(b.lt)))
                      .map((tx) => (
                        <div key={tx.lt} className={styles.detailCard}>
                          <TransactionDetails
                            tx={tx}
                            contracts={contracts}
                            allContracts={[]}
                            onContractClick={handleContractClick}
                          />
                        </div>
                      ))}
                  </div>
                )}
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

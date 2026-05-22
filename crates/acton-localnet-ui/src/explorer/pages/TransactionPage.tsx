import {
  ContractChip,
  type ContractData,
  fmt,
  TransactionDetails,
  type TransactionInfo,
  TransactionTree,
} from "@acton/shared-ui"
import {Address} from "@ton/core"
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
import {useEffect, useState} from "react"
import {useNavigate, useParams} from "react-router-dom"

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

interface ValueFlowItem {
  readonly address: string
  readonly before: bigint
  readonly after: bigint
  readonly change: bigint
  readonly fee: bigint
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

const collectSeqnoBounds = (
  processed: TransactionInfo[],
  transactionsByHex: Record<string, V3Transaction>,
) => {
  let minSeqno = Number.MAX_SAFE_INTEGER
  let maxSeqno = 0

  for (const t of processed) {
    const txHash = t.transaction.hash().toString("hex")
    const v3Tx = transactionsByHex[txHash]
    const seqno = v3Tx?.mc_block_seqno || 0

    if (seqno > 0) {
      minSeqno = Math.min(minSeqno, seqno)
      maxSeqno = Math.max(maxSeqno, seqno)
    }
  }

  return {minSeqno, maxSeqno}
}

export const TransactionPage: React.FC<TransactionPageProps> = ({client}) => {
  const {hash: routeHash = ""} = useParams<{hash: string}>()
  const hash = hashToHex(routeHash) ?? routeHash
  const navigate = useNavigate()
  const [loading, setLoading] = useState(true)
  const [traces, setTraces] = useState<TransactionInfo[]>([])
  const [contracts, setContracts] = useState<Map<string, ContractData>>(new Map())
  const [error, setError] = useState<string | undefined>()
  const [activeTab, setActiveTab] = useState<TabType>("value-flow")
  const [valueFlow, setValueFlow] = useState<ValueFlowItem[]>([])
  const [loadingFlow, setLoadingFlow] = useState(false)
  const {fetchName} = useAddressBook()
  const addressFormat = useAddressFormat()

  const handleContractClick = (address: string) => {
    const formattedAddr = normalizeAddress(address, addressFormat)
    void navigate(`/explorer/address/${encodeURIComponent(formattedAddr)}`)
  }

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

          const processed = buildTraceTransactionInfos(transactionsMap)
          if (!isActive) return
          setTraces(processed)

          const contractsMap = new Map<string, ContractData>()
          const addresses = new Set<string>()

          for (const t of processed) {
            if (t.address) addresses.add(t.address.toString())
          }
          const {minSeqno, maxSeqno} = collectSeqnoBounds(processed, transactionsByHex)

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
          const fetchedAbis = await Promise.all(
            codeHashes.map(async codeHash => {
              try {
                return [codeHash, await client.getCompilerAbi(codeHash)] as const
              } catch {
                return [codeHash, undefined] as const
              }
            }),
          )
          for (const [codeHash, abi] of fetchedAbis) {
            abiByCodeHash.set(codeHash, abi)
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

          if (addresses.size > 0 && minSeqno !== Number.MAX_SAFE_INTEGER) {
            setLoadingFlow(true)
            const flowItems: ValueFlowItem[] = []
            const uniqueAddrs = [...addresses]

            await Promise.all(
              uniqueAddrs.map(async addr => {
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
                    .filter(t => t.address?.toString() === addr)
                    .reduce((acc, t) => acc + t.transaction.totalFees.coins, 0n)

                  flowItems.push({
                    address: addr,
                    before,
                    after,
                    change: after - before,
                    fee: accountFees,
                  })
                } catch (error) {
                  console.warn(`Failed to fetch flow for ${addr}:`, error)
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
                        {valueFlow.map(item => (
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
                                ) : undefined}
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
                      .map(tx => (
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

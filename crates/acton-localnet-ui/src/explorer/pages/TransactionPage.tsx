import {useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState} from "react"
import type {CSSProperties, FC, JSX} from "react"
import {
  type ContractVerifiedSource,
  type ContractData,
  type LoadedTransactionActions,
  TransactionDetails,
  type TransactionInfo,
  TransactionTree,
  ValueFlowTable,
  buildValueFlowItems,
  decodeStorageDataCell,
  decodeStorageShardAccount,
  getTransactionComputePhase,
  type ValueFlowItem,
} from "@acton/shared-ui"
import {Address} from "@ton/core"
import {
  AlertCircle,
  ArrowLeft,
  Bug,
  CheckCircle2,
  CircleDotDashed,
  GitBranch,
  ListChecks,
  XCircle,
} from "lucide-react"
import {useNavigate, useParams, useSearchParams} from "react-router-dom"

import type {TonClient} from "../api/client"
import type {V3Action, V3Metadata} from "../api/types"
import {addressKey} from "../api/compilerAbi"
import {resolveCompilerAbis} from "../api/compilerAbiResolver"
import {buildTraceTransactionInfos} from "../api/traceTransactions"
import {ActionHistoryTable} from "../components/AccountDetails"
import {AddressChip} from "../components/AddressChip"
import {Breadcrumbs} from "../components/Breadcrumbs"
import {
  formatAddress as formatDisplayAddress,
  hashToHex,
  normalizeAddress,
} from "../components/utils"
import {useAddressBook} from "../hooks/useAddressBook"
import {useAvailableFlowMetrics} from "../hooks/useAvailableFlowMetrics"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useAddressFormat, useNetworkInfo} from "../hooks/useNetworkInfo"
import {openExplorerPath, type ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"
import {useMetadataRegistry} from "../metadata/MetadataRegistryProvider"
import type {ExplorerMetadataRegistry} from "../metadata/types"
import type {RetraceResultAndCode} from "../retrace/txTrace/lib/types"
import TransactionRetracePanel from "../retrace/txTrace/ui/TransactionRetracePanel"
import {useDelayedLoadingVisibility} from "../../hooks/useDelayedLoadingVisibility"

import styles from "./TransactionPage.module.css"

interface TransactionPageProps {
  readonly client: TonClient
  readonly openRetraceOnLoad?: boolean
}

type TabType = "transactions" | "value-flow" | "event-overview"

const parseTabType = (tab: string | null, supportsActions: boolean): TabType => {
  if (supportsActions && (tab === null || tab === "" || tab === "event-overview")) {
    return "event-overview"
  }
  return tab === "transactions" ? "transactions" : "value-flow"
}

const MAX_TRACE_TREE_FLOW_WIDTH = 1800
const WIDE_TRACE_TREE_TRANSACTION_THRESHOLD = 7

const normalizeTransactionReference = (reference: string): string => {
  return hashToHex(reference) ?? reference.trim().toLowerCase()
}

const transactionHashHex = (tx: TransactionInfo): string => tx.transaction.hash().toString("hex")

const transactionExecutionCodeHash = (tx: TransactionInfo): string | undefined =>
  tx.codeHashBefore ??
  tx.transaction.inMessage?.init?.code?.hash().toString("hex") ??
  tx.codeHashAfter

const transactionReferenceKeys = (tx: TransactionInfo): readonly string[] => {
  return [tx.id, transactionHashHex(tx), tx.lt, tx.transaction.lt.toString()].map(
    normalizeTransactionReference,
  )
}

interface TraceTransactionNodeProps {
  readonly tx: TransactionInfo
  readonly contracts: Map<string, ContractData>
  readonly compilerAbisByCodeHash: ReadonlyMap<string, ContractData["abi"]>
  readonly verifiedSourcesByCodeHash: ReadonlyMap<string, ContractVerifiedSource>
  readonly isIntermediateSibling?: boolean
  readonly onContractClick: (address: string) => void
  readonly loadActions: (tx: TransactionInfo) => Promise<LoadedTransactionActions>
}

const mapTraceTransactions = (
  transactions: readonly TransactionInfo[],
  updateTransaction: (tx: TransactionInfo) => TransactionInfo,
): TransactionInfo[] => {
  const clonedByOriginal = new Map<TransactionInfo, TransactionInfo>()

  for (const tx of transactions) {
    clonedByOriginal.set(tx, updateTransaction(tx))
  }

  for (const tx of transactions) {
    const clonedTx = clonedByOriginal.get(tx)
    if (!clonedTx) {
      continue
    }

    clonedTx.parent = tx.parent ? clonedByOriginal.get(tx.parent) : undefined
    clonedTx.children = tx.children
      .map(child => clonedByOriginal.get(child))
      .filter((child): child is TransactionInfo => child !== undefined)
  }

  return transactions
    .map(tx => clonedByOriginal.get(tx))
    .filter((tx): tx is TransactionInfo => tx !== undefined)
}

const withLoadedTransactionActions = (
  transactions: readonly TransactionInfo[],
  targetHash: string,
  loadedActions: LoadedTransactionActions,
): TransactionInfo[] => {
  const normalizedTargetHash = targetHash.toLowerCase()

  return mapTraceTransactions(transactions, tx => {
    const txHash = tx.transaction.hash().toString("hex").toLowerCase()
    if (txHash !== normalizedTargetHash) {
      return {...tx}
    }

    return {
      ...tx,
      actions: loadedActions.actions,
      outActions: loadedActions.outActions,
      executorActions: loadedActions.executorActions ?? tx.executorActions,
    }
  })
}

const withRetracedStorage = (
  transactions: readonly TransactionInfo[],
  targetHash: string,
  retraceResult: RetraceResultAndCode,
): TransactionInfo[] => {
  const normalizedTargetHash = targetHash.toLowerCase()

  return mapTraceTransactions(transactions, tx => {
    const txHash = tx.transaction.hash().toString("hex").toLowerCase()
    if (txHash !== normalizedTargetHash) {
      return {...tx}
    }

    const abi = tx.contractAbi
    const shardAccountBefore =
      tx.shardAccountBefore || retraceResult.result.account.shardAccountBefore
    const shardAccountAfter = tx.shardAccountAfter || retraceResult.result.account.shardAccountAfter

    return {
      ...tx,
      shardAccountBefore,
      shardAccountAfter,
      parsedStorageBefore:
        tx.parsedStorageBefore ??
        decodeStorageShardAccount(retraceResult.result.account.shardAccountBefore, abi),
      parsedStorageAfter:
        tx.parsedStorageAfter ??
        decodeStorageShardAccount(retraceResult.result.account.shardAccountAfter, abi),
    }
  })
}

async function loadVerifiedSourcesByCodeHash({
  metadataRegistry,
  codeHashes,
  shouldContinue,
}: {
  readonly metadataRegistry: ExplorerMetadataRegistry
  readonly codeHashes: readonly string[]
  readonly shouldContinue: () => boolean
}): Promise<Map<string, ContractVerifiedSource> | undefined> {
  const uniqueCodeHashes = [...new Set(codeHashes.filter(codeHash => codeHash.trim().length > 0))]
  if (uniqueCodeHashes.length === 0) {
    return new Map()
  }

  const sources = await Promise.all(
    uniqueCodeHashes.map(
      async (codeHash): Promise<readonly [string, ContractVerifiedSource] | undefined> => {
        try {
          const source = await metadataRegistry.getSource({codeHash})
          if (!source.verified || source.bundles.length === 0) {
            return undefined
          }
          return [codeHash, source] as const
        } catch (error) {
          console.debug(`Failed to fetch verified source for ${codeHash}`, error)
          return undefined
        }
      },
    ),
  )

  if (!shouldContinue()) {
    return undefined
  }

  return new Map(
    sources.filter(
      (entry): entry is readonly [string, ContractVerifiedSource] => entry !== undefined,
    ),
  )
}

export const TransactionPage: FC<TransactionPageProps> = ({client, openRetraceOnLoad = false}) => {
  const {hash: routeHash = ""} = useParams<{hash: string}>()
  const hash = hashToHex(routeHash) ?? routeHash
  const navigate = useNavigate()
  const routes = useExplorerRoutePaths()
  const [searchParams, setSearchParams] = useSearchParams()
  const [loading, setLoading] = useState(true)
  const [traces, setTraces] = useState<TransactionInfo[]>([])
  const [contracts, setContracts] = useState<Map<string, ContractData>>(new Map())
  const [compilerAbisByCodeHash, setCompilerAbisByCodeHash] = useState<
    Map<string, ContractData["abi"]>
  >(new Map())
  const [verifiedSourcesByCodeHash, setVerifiedSourcesByCodeHash] = useState<
    Map<string, ContractVerifiedSource>
  >(new Map())
  const [error, setError] = useState<string | undefined>()
  const {fetchName} = useAddressBook()
  const {network} = useNetworkInfo()
  const metadataRegistry = useMetadataRegistry()
  const addressFormat = useAddressFormat()
  const [traceLookupHash, setTraceLookupHash] = useState(hash)
  const supportsTraceActions = client.usesToncenterApiEndpoint() && network.supportsActions
  const [activeTab, setActiveTab] = useState<TabType>(() =>
    parseTabType(searchParams.get("tab"), supportsTraceActions),
  )
  const [expandedRetraceHash, setExpandedRetraceHash] = useState<string | undefined>()
  const [retraceAttempt, setRetraceAttempt] = useState(0)
  const [valueFlow, setValueFlow] = useState<ValueFlowItem[]>([])
  const [traceActions, setTraceActions] = useState<readonly V3Action[]>([])
  const [traceActionMetadata, setTraceActionMetadata] = useState<V3Metadata>({})
  const [hoveredAction, setHoveredAction] = useState<V3Action | undefined>()
  const [nowSeconds, setNowSeconds] = useState(() => Math.floor(Date.now() / 1000))
  const fetchNameRef = useRef(fetchName)
  const addressFormatRef = useRef(addressFormat)
  const loadedActionsByHashRef = useRef(new Map<string, LoadedTransactionActions>())
  const showLoadingSkeleton = useDelayedLoadingVisibility(loading, 500)
  const {flowMetrics: treeFlowMetrics, rootRef: treeSectionRef} =
    useAvailableFlowMetrics<HTMLDivElement>(MAX_TRACE_TREE_FLOW_WIDTH)
  const selectedTraceTransaction = useMemo(() => {
    const requestedHash = hash.toLowerCase()
    return traces.find(tx => transactionHashHex(tx).toLowerCase() === requestedHash)
  }, [hash, traces])
  const selectedTransactionId = selectedTraceTransaction?.id
  const highlightedTransactionIds = useMemo(() => {
    if (!hoveredAction) {
      return undefined
    }

    const actionTransactionReferences = new Set(
      hoveredAction.transactions.map(normalizeTransactionReference),
    )
    const highlightedIds = new Set<string>()
    for (const tx of traces) {
      if (transactionReferenceKeys(tx).some(key => actionTransactionReferences.has(key))) {
        highlightedIds.add(tx.id)
      }
    }

    return highlightedIds.size > 0 ? highlightedIds : undefined
  }, [hoveredAction, traces])
  const isWideTraceTree = traces.length > WIDE_TRACE_TREE_TRANSACTION_THRESHOLD
  const treeSectionStyle = useMemo<CSSProperties | undefined>(() => {
    if (!isWideTraceTree) {
      return undefined
    }

    return {
      "--trace-tree-flow-width": treeFlowMetrics.width > 0 ? `${treeFlowMetrics.width}px` : "100%",
      "--trace-tree-flow-offset": `${treeFlowMetrics.offset}px`,
    } as CSSProperties
  }, [isWideTraceTree, treeFlowMetrics])

  fetchNameRef.current = fetchName
  addressFormatRef.current = addressFormat

  const handleContractClick = (address: string, event?: ExplorerNavigationClickEvent) => {
    const formattedAddr = normalizeAddress(address, addressFormat)
    openExplorerPath(navigate, routes.addressPath(formattedAddr), event)
  }

  const renderTraceAddressChip = (
    address: string,
    options: {readonly shorten: boolean},
  ): JSX.Element => (
    <AddressChip
      address={address}
      onAddressClick={handleContractClick}
      resolveName={false}
      shorten={options.shorten}
    />
  )

  const handleActiveTabChange = (tab: TabType) => {
    setActiveTab(tab)
    setHoveredAction(undefined)
    setSearchParams(
      currentSearchParams => {
        const nextSearchParams = new URLSearchParams(currentSearchParams)
        nextSearchParams.set("tab", tab)
        return nextSearchParams
      },
      {replace: true},
    )
  }

  const handleRetrace = (txHash: string) => {
    setExpandedRetraceHash(txHash)
    setRetraceAttempt(currentAttempt => currentAttempt + 1)
  }

  const handleCloseRetrace = () => {
    setExpandedRetraceHash(undefined)
    if (openRetraceOnLoad) {
      void navigate(routes.transactionPath(hash), {replace: true})
    }
  }

  const handleTransactionSelect = useCallback(
    (tx: TransactionInfo) => {
      const txHash = transactionHashHex(tx)
      if (txHash.toLowerCase() === hash.toLowerCase()) {
        return
      }

      const search = searchParams.toString()
      const path = openRetraceOnLoad
        ? routes.transactionTracePath(txHash)
        : routes.transactionPath(txHash)
      void navigate(search ? `${path}?${search}` : path)
    },
    [hash, navigate, openRetraceOnLoad, routes, searchParams],
  )

  const loadTransactionActions = useCallback(
    async (tx: TransactionInfo): Promise<LoadedTransactionActions> => {
      const txHash = tx.transaction.hash().toString("hex").toLowerCase()
      const cachedActions = loadedActionsByHashRef.current.get(txHash)
      if (cachedActions) {
        return cachedActions
      }

      const {traceTx} = await import("../retrace/txTrace/lib/traceTx")
      const retraceResult = await traceTx(txHash, network, metadataRegistry, {
        codeHash: transactionExecutionCodeHash(tx),
      })
      const loadedActions: LoadedTransactionActions = {
        actions: retraceResult.result.emulatedTx.c5,
        outActions: retraceResult.result.emulatedTx.actions,
        executorActions: tx.executorActions,
      }

      loadedActionsByHashRef.current.set(txHash, loadedActions)
      setTraces(currentTraces =>
        withRetracedStorage(
          withLoadedTransactionActions(currentTraces, txHash, loadedActions),
          txHash,
          retraceResult,
        ),
      )

      return loadedActions
    },
    [metadataRegistry, network],
  )

  const handleRetraceResult = useCallback((txHash: string, result: RetraceResultAndCode) => {
    setTraces(currentTraces => withRetracedStorage(currentTraces, txHash, result))
  }, [])

  useEffect(() => {
    setActiveTab(parseTabType(searchParams.get("tab"), supportsTraceActions))
  }, [searchParams, supportsTraceActions])

  useEffect(() => {
    if (activeTab !== "event-overview" || traceActions.length === 0) {
      return
    }

    const updateNow = () => setNowSeconds(Math.floor(Date.now() / 1000))
    updateNow()

    const interval = globalThis.setInterval(updateNow, 5000)
    return () => globalThis.clearInterval(interval)
  }, [activeTab, traceActions.length])

  useEffect(() => {
    if (!hash || traceLookupHash.toLowerCase() === hash.toLowerCase() || selectedTraceTransaction) {
      return
    }

    setTraceLookupHash(hash)
  }, [hash, selectedTraceTransaction, traceLookupHash])

  useEffect(() => {
    setExpandedRetraceHash(undefined)
    setRetraceAttempt(0)
    setHoveredAction(undefined)
    loadedActionsByHashRef.current.clear()
  }, [traceLookupHash])

  useEffect(() => {
    if (!openRetraceOnLoad || !selectedTraceTransaction) {
      return
    }

    setExpandedRetraceHash(transactionHashHex(selectedTraceTransaction))
  }, [openRetraceOnLoad, selectedTraceTransaction])

  useEffect(() => {
    if (!traceLookupHash) return

    let isActive = true

    const fetchTrace = async () => {
      setLoading(true)
      setError(undefined)
      setVerifiedSourcesByCodeHash(new Map())
      setTraceActions([])
      setTraceActionMetadata({})
      setHoveredAction(undefined)
      try {
        const data = await client.getTraces(traceLookupHash, {
          includeActions: supportsTraceActions,
        })
        if (!isActive) return

        if (data.traces && data.traces.length > 0) {
          const trace = data.traces[0]
          const transactionsMap = trace.transactions
          const transactionsByLt = new Map(
            Object.values(transactionsMap).map(tx => [tx.lt, tx] as const),
          )

          const processed = buildTraceTransactionInfos(transactionsMap, trace.trace)

          const contractsMap = new Map<string, ContractData>()
          const traceAddressOrder = collectTraceAddressOrder(processed)
          const requestedAddresses = [...traceAddressOrder].sort()
          const additionalCodeHashes = new Set<string>()
          for (const tx of Object.values(transactionsMap)) {
            if (tx.account_state_before?.code_hash) {
              additionalCodeHashes.add(tx.account_state_before.code_hash)
            }
            if (tx.account_state_after?.code_hash) {
              additionalCodeHashes.add(tx.account_state_after.code_hash)
            }
          }
          const stateInitCodeHashes = new Set<string>()
          for (const tx of processed) {
            const stateInitCodeHash = tx.transaction.inMessage?.init?.code?.hash().toString("hex")
            if (stateInitCodeHash) {
              additionalCodeHashes.add(stateInitCodeHash)
              stateInitCodeHashes.add(stateInitCodeHash)
            }
          }

          const resolvedAbis = await resolveCompilerAbis({
            client,
            metadataRegistry,
            addresses: requestedAddresses,
            additionalCodeHashes: [...additionalCodeHashes],
            shouldContinue: () => isActive,
          })
          if (!resolvedAbis) {
            return
          }
          const {addressToCodeHash, abiByCodeHash} = resolvedAbis
          const stateInitVerifiedSources = await loadVerifiedSourcesByCodeHash({
            metadataRegistry,
            codeHashes: [...stateInitCodeHashes],
            shouldContinue: () => isActive,
          })
          if (!stateInitVerifiedSources) {
            return
          }

          for (const tx of processed) {
            const sourceTx = transactionsByLt.get(tx.lt)
            const fallbackCodeHash = tx.address
              ? addressToCodeHash.get(addressKey(tx.address.toString()))
              : undefined
            const beforeCodeHash = sourceTx?.account_state_before?.code_hash ?? fallbackCodeHash
            const afterCodeHash = sourceTx?.account_state_after?.code_hash ?? fallbackCodeHash
            const contractCodeHash = beforeCodeHash ?? afterCodeHash
            tx.contractAbi = contractCodeHash
              ? (abiByCodeHash.get(contractCodeHash) ?? undefined)
              : undefined
            tx.parsedStorageBefore = decodeStorageDataCell(
              sourceTx?.account_state_before?.data_boc,
              beforeCodeHash ? abiByCodeHash.get(beforeCodeHash) : undefined,
            )
            tx.parsedStorageAfter = decodeStorageDataCell(
              sourceTx?.account_state_after?.data_boc,
              afterCodeHash ? abiByCodeHash.get(afterCodeHash) : undefined,
            )
          }

          await Promise.all(
            traceAddressOrder.map(async (addr, index) => {
              const letter = String.fromCodePoint(65 + index)
              const displayAddr = normalizeAddress(addr, addressFormatRef.current)
              const customName = await fetchNameRef.current(addr)
              const abi = abiByCodeHash.get(addressToCodeHash.get(addressKey(addr)) ?? "")
              contractsMap.set(addr, {
                displayName:
                  customName || formatDisplayAddress(displayAddr, true, addressFormatRef.current),
                address: Address.parse(addr),
                letter,
                abi,
              })
            }),
          )

          const nextValueFlow = buildValueFlowItems(processed)
          if (isActive) {
            setTraces(processed)
            setContracts(contractsMap)
            setCompilerAbisByCodeHash(new Map(abiByCodeHash))
            setVerifiedSourcesByCodeHash(stateInitVerifiedSources)
            setValueFlow(nextValueFlow)
            setTraceActions(trace.actions ?? [])
            setTraceActionMetadata(data.metadata)
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
  }, [client, metadataRegistry, traceLookupHash, supportsTraceActions])

  if (loading) {
    return showLoadingSkeleton ? (
      <TransactionTraceSkeleton activeTab={activeTab} showEventOverview={supportsTraceActions} />
    ) : null
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
  const firstTraceComputePhase = firstTrace
    ? getTransactionComputePhase(firstTrace.transaction)
    : undefined
  const firstTraceSucceeded =
    firstTraceComputePhase?.type === "vm" && firstTraceComputePhase.success
  const traceAddress = firstTrace?.address?.toString() ?? ""
  const traceAddressDisplay = normalizeAddress(traceAddress, addressFormat)
  const rootTraceTransactions = [...traces]
    .filter(tx => !tx.parent)
    .sort(compareTransactionInfoByLt)
  const renderSelectedTransactionMessageRouteAction = (tx: TransactionInfo): JSX.Element => {
    const txHash = transactionHashHex(tx)
    const isRetraceOpen = expandedRetraceHash === txHash

    return (
      <button
        type="button"
        className={`${styles.retraceInlineButton} ${isRetraceOpen ? styles.retraceInlineButtonActive : ""}`}
        onClick={() => handleRetrace(txHash)}
        aria-expanded={isRetraceOpen}
      >
        <Bug size={14} />
        Debug
      </button>
    )
  }

  const renderSelectedTransactionExtra = (tx: TransactionInfo): JSX.Element | null => {
    const txHash = transactionHashHex(tx)
    if (expandedRetraceHash !== txHash) {
      return null
    }

    return (
      <div className={styles.selectedRetraceSection}>
        <TransactionRetracePanel
          key={`${txHash}:${retraceAttempt}`}
          metadataRegistry={metadataRegistry}
          txHash={txHash}
          codeHash={transactionExecutionCodeHash(tx)}
          contractAbi={tx.contractAbi}
          contracts={contracts}
          onClose={handleCloseRetrace}
          onContractClick={handleContractClick}
          onResult={handleRetraceResult}
        />
      </div>
    )
  }

  return (
    <div className={styles.container}>
      <div className={styles.content}>
        {traces.length > 0 && (
          <>
            <Breadcrumbs
              items={[
                {
                  label: traceAddressDisplay,
                  path: routes.addressPath(traceAddressDisplay),
                  isAddress: true,
                },
                {label: hash, isHash: true},
              ]}
            />
            <div className={styles.preTreeContent}>
              <div className={styles.overviewCard}>
                <div className={styles.overviewHeader}>
                  <div
                    className={`${styles.status} ${firstTraceSucceeded ? styles.statusSuccess : styles.statusError}`}
                  >
                    {firstTraceSucceeded ? (
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

              <div
                className={`${styles.tabsContainer} ${activeTab === "transactions" ? styles.tabsContainerDetached : ""}`}
              >
                <TraceTabs
                  activeTab={activeTab}
                  showEventOverview={supportsTraceActions}
                  onTabChange={handleActiveTabChange}
                />

                <div className={styles.tabContent}>
                  {activeTab === "value-flow" && (
                    <ValueFlowTable
                      items={valueFlow}
                      contracts={contracts}
                      onContractClick={handleContractClick}
                      className={styles.valueFlowPanel}
                    />
                  )}

                  {activeTab === "event-overview" && supportsTraceActions && (
                    <ActionHistoryTable
                      actions={traceActions}
                      actionMetadata={traceActionMetadata}
                      ownerAddress={traceAddress}
                      client={client}
                      nowSeconds={nowSeconds}
                      emptyState="No actions found"
                      showTimeColumn={false}
                      interactiveRows={false}
                      onAddressClick={handleContractClick}
                      onActionHoverChange={setHoveredAction}
                    />
                  )}

                  {activeTab === "transactions" && (
                    <div className={styles.detailsList}>
                      {rootTraceTransactions.map(tx => (
                        <TraceTransactionNode
                          key={tx.id}
                          tx={tx}
                          contracts={contracts}
                          compilerAbisByCodeHash={compilerAbisByCodeHash}
                          verifiedSourcesByCodeHash={verifiedSourcesByCodeHash}
                          onContractClick={handleContractClick}
                          loadActions={loadTransactionActions}
                        />
                      ))}
                    </div>
                  )}
                </div>
              </div>
            </div>

            <div
              ref={treeSectionRef}
              className={`${styles.treeSection} ${isWideTraceTree ? styles.treeSectionWide : ""}`}
              style={treeSectionStyle}
            >
              <TransactionTree
                transactions={traces}
                contracts={contracts}
                compilerAbisByCodeHash={compilerAbisByCodeHash}
                verifiedSourcesByCodeHash={verifiedSourcesByCodeHash}
                allContracts={[]}
                selectedTransactionId={selectedTransactionId}
                highlightedTransactionIds={highlightedTransactionIds}
                onContractClick={handleContractClick}
                onTransactionSelect={handleTransactionSelect}
                renderAddressChip={renderTraceAddressChip}
                renderSelectedTransactionExtra={renderSelectedTransactionExtra}
                renderSelectedTransactionMessageRouteAction={
                  renderSelectedTransactionMessageRouteAction
                }
                loadActions={loadTransactionActions}
              />
            </div>
          </>
        )}
      </div>
    </div>
  )
}

interface TraceTabsProps {
  readonly activeTab: TabType
  readonly disabled?: boolean
  readonly showEventOverview?: boolean
  readonly onTabChange?: (tab: TabType) => void
}

function TraceTabs({
  activeTab,
  disabled = false,
  showEventOverview = false,
  onTabChange,
}: TraceTabsProps): JSX.Element {
  return (
    <div className={styles.tabs} aria-hidden={disabled ? "true" : undefined}>
      {showEventOverview && (
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "event-overview" ? styles.tabActive : ""}`}
          onClick={() => onTabChange?.("event-overview")}
          disabled={disabled}
          tabIndex={disabled ? -1 : undefined}
        >
          <ListChecks size={16} /> Event Overview
        </button>
      )}
      <button
        type="button"
        className={`${styles.tab} ${activeTab === "value-flow" ? styles.tabActive : ""}`}
        onClick={() => onTabChange?.("value-flow")}
        disabled={disabled}
        tabIndex={disabled ? -1 : undefined}
      >
        <CircleDotDashed size={16} /> Value Flow
      </button>
      <button
        type="button"
        className={`${styles.tab} ${activeTab === "transactions" ? styles.tabActive : ""}`}
        onClick={() => onTabChange?.("transactions")}
        disabled={disabled}
        tabIndex={disabled ? -1 : undefined}
      >
        <GitBranch size={16} /> Transactions
      </button>
    </div>
  )
}

interface TransactionTraceSkeletonProps {
  readonly activeTab: TabType
  readonly showEventOverview?: boolean
}

function TransactionTraceSkeleton({
  activeTab,
  showEventOverview = false,
}: TransactionTraceSkeletonProps): JSX.Element {
  return (
    <div className={styles.container} aria-label="Loading transaction trace">
      <div className={styles.content}>
        <div className={styles.skeletonBreadcrumbs}>
          <span className={`${styles.skeleton} ${styles.skeletonBreadcrumbAddress}`} />
          <span className={`${styles.skeleton} ${styles.skeletonBreadcrumbHash}`} />
        </div>

        <div className={styles.preTreeContent}>
          <div className={styles.overviewCard}>
            <div className={styles.overviewHeader}>
              <div className={styles.skeletonStatus}>
                <span className={`${styles.skeleton} ${styles.skeletonStatusIcon}`} />
                <span className={`${styles.skeleton} ${styles.skeletonStatusText}`} />
              </div>
              <span className={`${styles.skeleton} ${styles.skeletonTime}`} />
            </div>
          </div>

          <div
            className={`${styles.tabsContainer} ${activeTab === "transactions" ? styles.tabsContainerDetached : ""}`}
          >
            <TraceTabs activeTab={activeTab} showEventOverview={showEventOverview} disabled />

            <div className={styles.tabContent}>
              {activeTab === "transactions" ? (
                <TraceDetailsSkeleton />
              ) : activeTab === "event-overview" ? (
                <ActionHistorySkeleton />
              ) : (
                <ValueFlowSkeleton />
              )}
            </div>
          </div>
        </div>

        <div className={styles.treeSection}>
          <TraceTreeSkeleton />
        </div>
      </div>
    </div>
  )
}

function ActionHistorySkeleton(): JSX.Element {
  return (
    <div className={styles.skeletonEventCard} aria-hidden="true">
      <div className={styles.skeletonEventHeader}>
        <span>Action</span>
        <span>Address</span>
        <span />
        <span>Value</span>
      </div>
      {[0, 1, 2].map(index => (
        <div key={`event-overview-skeleton-${index}`} className={styles.skeletonEventRow}>
          <div className={styles.skeletonEventAction}>
            <span className={`${styles.skeleton} ${styles.skeletonEventIcon}`} />
            <span className={`${styles.skeleton} ${styles.skeletonEventActionText}`} />
          </div>
          <div className={styles.skeletonEventAddress}>
            <span className={`${styles.skeleton} ${styles.skeletonEventDirection}`} />
            <span className={`${styles.skeleton} ${styles.skeletonEventAddressText}`} />
          </div>
          <div className={styles.skeletonEventTechnical}>
            {index === 1 && <span className={`${styles.skeleton} ${styles.skeletonEventOpcode}`} />}
          </div>
          <div className={styles.skeletonEventValue}>
            <span className={`${styles.skeleton} ${styles.skeletonEventValueText}`} />
          </div>
        </div>
      ))}
    </div>
  )
}

function ValueFlowSkeleton(): JSX.Element {
  return (
    <div className={styles.skeletonFlowCard} aria-hidden="true">
      <div className={styles.skeletonFlowHeader}>
        <span>Account</span>
        <span>Balance Change</span>
        <span>Network Fee</span>
      </div>
      {[0, 1].map(index => (
        <div key={`flow-skeleton-${index}`} className={styles.skeletonFlowRow}>
          <div className={styles.skeletonFlowAccount}>
            <span className={`${styles.skeleton} ${styles.skeletonFlowAvatar}`} />
            <span className={styles.skeletonFlowAccountText}>
              <span className={`${styles.skeleton} ${styles.skeletonFlowAccountName}`} />
              <span className={`${styles.skeleton} ${styles.skeletonFlowAccountAddress}`} />
            </span>
          </div>
          <div className={styles.skeletonFlowMetric}>
            <span className={`${styles.skeleton} ${styles.skeletonFlowAmount}`} />
          </div>
          <div className={styles.skeletonFlowMetric}>
            <span className={`${styles.skeleton} ${styles.skeletonFlowFee}`} />
          </div>
        </div>
      ))}
      <div className={styles.skeletonFlowFooter}>
        <span className={`${styles.skeleton} ${styles.skeletonFlowTotal}`} />
      </div>
    </div>
  )
}

function TraceDetailsSkeleton(): JSX.Element {
  return (
    <div className={styles.detailsList} aria-hidden="true">
      {[0, 1].map(index => (
        <div key={`trace-details-skeleton-${index}`} className={styles.skeletonDetailCard}>
          {[0, 1, 2, 3].map(rowIndex => (
            <div
              key={`trace-details-skeleton-${index}-${rowIndex}`}
              className={styles.skeletonDetailRow}
            >
              <span className={`${styles.skeleton} ${styles.skeletonDetailLabel}`} />
              <span className={`${styles.skeleton} ${styles.skeletonDetailValue}`} />
            </div>
          ))}
        </div>
      ))}
    </div>
  )
}

function TraceTreeSkeleton(): JSX.Element {
  return (
    <div className={styles.skeletonTree} aria-hidden="true">
      <div className={`${styles.skeletonTreeNode} ${styles.skeletonTreeNodeRoot}`}>
        <span className={`${styles.skeleton} ${styles.skeletonTreeDot}`} />
        <span className={`${styles.skeleton} ${styles.skeletonTreeLabel}`} />
      </div>
      <div className={styles.skeletonTreeBranch}>
        <div className={styles.skeletonTreeRail} />
        {[0, 1].map(index => (
          <div key={`trace-tree-skeleton-${index}`} className={styles.skeletonTreeNode}>
            <span className={`${styles.skeleton} ${styles.skeletonTreeDot}`} />
            <span className={`${styles.skeleton} ${styles.skeletonTreeLabel}`} />
          </div>
        ))}
      </div>
    </div>
  )
}

const TraceTransactionNode: FC<TraceTransactionNodeProps> = ({
  tx,
  contracts,
  compilerAbisByCodeHash,
  verifiedSourcesByCodeHash,
  isIntermediateSibling = false,
  onContractClick,
  loadActions,
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
            compilerAbisByCodeHash={compilerAbisByCodeHash}
            verifiedSourcesByCodeHash={verifiedSourcesByCodeHash}
            allContracts={[]}
            onContractClick={onContractClick}
            loadActions={loadActions}
          />
        </div>
        {children.length > 0 && (
          <div
            className={styles.traceConnectorAnchor}
            style={
              {
                "--trace-connector-height": `${connectorHeight}px`,
              } as CSSProperties
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
              compilerAbisByCodeHash={compilerAbisByCodeHash}
              verifiedSourcesByCodeHash={verifiedSourcesByCodeHash}
              isIntermediateSibling={index < children.length - 1}
              onContractClick={onContractClick}
              loadActions={loadActions}
            />
          ))}
        </div>
      )}
    </div>
  )
}

function collectTraceAddressOrder(processed: readonly TransactionInfo[]): readonly string[] {
  const addresses = new Set<string>()

  const visit = (tx: TransactionInfo) => {
    const address = tx.address?.toString()
    if (address) {
      addresses.add(address)
    }

    for (const child of [...tx.children].sort(compareTransactionInfoByLt)) {
      visit(child)
    }
  }

  for (const tx of [...processed].filter(tx => !tx.parent).sort(compareTransactionInfoByLt)) {
    visit(tx)
  }

  return [...addresses]
}

function compareTransactionInfoByLt(left: TransactionInfo, right: TransactionInfo): number {
  const leftLt = parseBigInt(left.lt)
  const rightLt = parseBigInt(right.lt)
  if (leftLt === rightLt) {
    return 0
  }
  return leftLt < rightLt ? -1 : 1
}

function parseBigInt(value: string | undefined): bigint {
  try {
    return value === undefined ? 0n : BigInt(value)
  } catch {
    return 0n
  }
}

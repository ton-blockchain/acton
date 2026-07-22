import {useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState} from "react"
import type {ComponentProps, CSSProperties, FC, JSX} from "react"
import {
  type ContractVerifiedSource,
  type ContractData,
  type LoadedTransactionActions,
  type ResolveVerifiedSourceByCodeHash,
  type TransactionBlockRef,
  TransactionDetails,
  type TransactionInfo,
  TransactionTree,
  ValueFlowTable,
  decodeTransactionMessageBody,
  decodeStorageShardAccount,
  getTransactionComputePhase,
  type ValueFlowItem,
} from "@acton/transaction-ui"
import {InlineAction} from "@acton/ui"
import {
  AlertCircle,
  ArrowLeft,
  Bug,
  CheckCircle2,
  CircleDotDashed,
  Database,
  GitBranch,
  Info,
  ListChecks,
  Star,
  XCircle,
} from "lucide-react"
import {useNavigate, useParams, useSearchParams} from "react-router-dom"

import type {TonClient} from "../api/client"
import type {V3Action, V3Metadata, V3Trace} from "../api/types"
import {buildTraceTransactionInfos} from "../api/traceTransactions"
import {ActionHistoryTable} from "../components/AccountDetails"
import {ExplorerAddressChip} from "../components/ExplorerAddressChip"
import {ExplorerBreadcrumbs} from "../components/ExplorerBreadcrumbs"
import {TraceStateChangesPanel} from "../components/TraceStateChangesPanel"
import {TraceOverviewTable, type TraceOverviewData} from "../components/TraceOverviewTable"
import {hashToHex, normalizeAddress} from "../components/utils"
import {useAddressBook} from "../hooks/useAddressBook"
import {useAvailableFlowMetrics} from "../hooks/useAvailableFlowMetrics"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useFavoriteTransactions} from "../hooks/useFavoriteTransactions"
import {useAddressFormat, useNetworkInfo} from "../hooks/useNetworkInfo"
import {openExplorerPath, type ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"
import {useMetadataRegistry} from "../metadata/MetadataRegistryProvider"
import type {ExplorerMetadataRegistry} from "../metadata/types"
import type {RetraceResultAndCode, RetraceTraceResult} from "../retrace/txTrace/lib/types"
import TransactionRetracePanel from "../retrace/txTrace/ui/TransactionRetracePanel"
import {useDelayedLoadingVisibility} from "../../hooks/useDelayedLoadingVisibility"
import {enrichTraceTransactions} from "./transactionTraceEnrichment"

import styles from "./TransactionPage.module.css"

interface TransactionPageProps {
  readonly client: TonClient
  readonly openRetraceOnLoad?: boolean
}

export type TransactionTraceTabType =
  | "transactions"
  | "value-flow"
  | "event-overview"
  | "state-changes"
  | "details"

export const parseTransactionTraceTabType = (
  tab: string | null,
  supportsActions: boolean,
): TransactionTraceTabType => {
  if (tab === "details" || tab === "overview") {
    return "details"
  }
  if (tab === "state-changes" || tab === "transactions") {
    return tab
  }
  if (supportsActions && (tab === null || tab === "" || tab === "event-overview")) {
    return "event-overview"
  }
  return "value-flow"
}

const MAX_TRACE_TREE_FLOW_WIDTH = 1800
const WIDE_TRACE_TREE_TRANSACTION_THRESHOLD = 7

const normalizeTransactionReference = (reference: string): string => {
  return hashToHex(reference) ?? reference.trim().toLowerCase()
}

export const transactionHashHex = (tx: TransactionInfo): string =>
  tx.transaction.hash().toString("hex")

export const traceOverviewDataFromTrace = (trace: V3Trace): TraceOverviewData => ({
  traceId: trace.trace_id,
  externalHash: trace.external_hash ?? undefined,
  masterchainSeqnoStart: trace.mc_seqno_start,
  masterchainSeqnoEnd: trace.mc_seqno_end,
  startLt: trace.start_lt,
  endLt: trace.end_lt,
  startUtime: trace.start_utime,
  endUtime: trace.end_utime,
  isIncomplete: trace.is_incomplete,
  transactionCount: trace.trace_info.transactions,
  messageCount: trace.trace_info.messages,
  pendingMessageCount: trace.trace_info.pending_messages,
  traceState: trace.trace_info.trace_state,
})

const blockPath = (blockRef: TransactionBlockRef): string =>
  `/block/${blockRef.workchain}/${encodeURIComponent(blockRef.shard)}/${blockRef.seqno}`

export const transactionExecutionCodeHash = (tx: TransactionInfo): string | undefined =>
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
  readonly getBlockPath: (blockRef: TransactionBlockRef) => string | undefined
  readonly onBlockClick: (
    blockRef: TransactionBlockRef,
    event: ExplorerNavigationClickEvent,
  ) => void
  readonly loadActions?: (tx: TransactionInfo) => Promise<LoadedTransactionActions>
  readonly resolveVerifiedSourceByCodeHash?: ResolveVerifiedSourceByCodeHash
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

const withRetracedTraceStorage = (
  transactions: readonly TransactionInfo[],
  retracedTransactions: RetraceTraceResult["result"]["transactions"],
): TransactionInfo[] => {
  return mapTraceTransactions(transactions, tx => {
    const retracedTx = retracedTransactions[transactionHashHex(tx).toLowerCase()]
    if (!retracedTx) {
      return {...tx}
    }

    const parsedStorageBefore = decodeStorageShardAccount(
      retracedTx.account.shardAccountBefore,
      tx.contractAbi,
    )
    const parsedStorageAfter = decodeStorageShardAccount(
      retracedTx.account.shardAccountAfter,
      tx.contractAbi,
    )

    return {
      ...tx,
      vmLogDiff: retracedTx.emulatedTx.vmLogs,
      executorLogs: retracedTx.emulatedTx.executorLogs,
      actions: retracedTx.emulatedTx.c5,
      outActions: retracedTx.emulatedTx.actions,
      shardAccountBefore: retracedTx.account.shardAccountBefore,
      shardAccountAfter: retracedTx.account.shardAccountAfter,
      accountBalanceBefore: retracedTx.money.balanceBefore,
      accountBalanceAfter: retracedTx.money.balanceAfter,
      parsedStorageBefore: parsedStorageBefore ?? tx.parsedStorageBefore,
      parsedStorageAfter: parsedStorageAfter ?? tx.parsedStorageAfter,
    }
  })
}

async function loadVerifiedSourceByCodeHash(
  metadataRegistry: ExplorerMetadataRegistry,
  codeHash: string,
): Promise<ContractVerifiedSource | undefined> {
  try {
    const source = await metadataRegistry.getSource({codeHash})
    return source.verified && source.bundles.length > 0 ? source : undefined
  } catch (error) {
    console.debug(`Failed to fetch verified source for ${codeHash}`, error)
    return undefined
  }
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
  const {fetchName, updateDomains} = useAddressBook()
  const {network} = useNetworkInfo()
  const {isFavorite, toggleFavorite} = useFavoriteTransactions()
  const metadataRegistry = useMetadataRegistry()
  const resolveVerifiedSourceByCodeHash = useCallback(
    (codeHash: string) => loadVerifiedSourceByCodeHash(metadataRegistry, codeHash),
    [metadataRegistry],
  )
  const addressFormat = useAddressFormat()
  const [traceLookupHash, setTraceLookupHash] = useState(hash)
  const supportsTraceActions = client.usesToncenterApiEndpoint() && network.supportsActions
  const [activeTab, setActiveTab] = useState<TransactionTraceTabType>(() =>
    parseTransactionTraceTabType(searchParams.get("tab"), supportsTraceActions),
  )
  const [expandedRetraceHash, setExpandedRetraceHash] = useState<string | undefined>()
  const [retraceAttempt, setRetraceAttempt] = useState(0)
  const [valueFlow, setValueFlow] = useState<ValueFlowItem[]>([])
  const [traceActions, setTraceActions] = useState<readonly V3Action[]>([])
  const [traceActionMetadata, setTraceActionMetadata] = useState<V3Metadata>({})
  const [traceOverview, setTraceOverview] = useState<TraceOverviewData | undefined>()
  const [hoveredAction, setHoveredAction] = useState<V3Action | undefined>()
  const [stateChangesStatus, setStateChangesStatus] = useState<{
    readonly traceHash?: string
    readonly isLoading: boolean
    readonly error?: string
  }>({isLoading: false})
  const [nowSeconds, setNowSeconds] = useState(() => Math.floor(Date.now() / 1000))
  const fetchNameRef = useRef(fetchName)
  const addressFormatRef = useRef(addressFormat)
  const loadedActionsByHashRef = useRef(new Map<string, LoadedTransactionActions>())
  const stateChangesRequestedHashRef = useRef<string | undefined>(undefined)
  const showLoadingSkeleton = useDelayedLoadingVisibility(loading, 500)
  const selectedTraceTransaction = useMemo(() => {
    const requestedHash = hash.toLowerCase()
    return traces.find(tx => transactionHashHex(tx).toLowerCase() === requestedHash)
  }, [hash, traces])
  const favorite = isFavorite(hash)
  const currentStateChangesStatus =
    stateChangesStatus.traceHash === traceLookupHash.toLowerCase() ? stateChangesStatus : undefined

  // oxlint-disable-next-line react-doctor/no-ref-current-in-render -- keeps async trace loading on the latest address-book lookup without restarting it
  fetchNameRef.current = fetchName
  // oxlint-disable-next-line react-doctor/no-ref-current-in-render -- keeps async trace loading on the latest address format without restarting it
  addressFormatRef.current = addressFormat

  const handleContractClick = (address: string, event?: ExplorerNavigationClickEvent) => {
    const formattedAddr = normalizeAddress(address, addressFormat)
    openExplorerPath(navigate, routes.addressPath(formattedAddr), event)
  }
  const handleBlockClick = (
    blockRef: TransactionBlockRef,
    event?: ExplorerNavigationClickEvent,
  ) => {
    openExplorerPath(navigate, blockPath(blockRef), event)
  }

  const handleActiveTabChange = (tab: TransactionTraceTabType) => {
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

  const handleToggleFavorite = useCallback(() => {
    if (!selectedTraceTransaction) {
      return
    }

    toggleFavorite({
      hash: transactionHashHex(selectedTraceTransaction),
      account: selectedTraceTransaction.address?.toString(),
      lt: selectedTraceTransaction.lt,
      timestamp: selectedTraceTransaction.transaction.now,
    })
  }, [selectedTraceTransaction, toggleFavorite])

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
    setActiveTab(parseTransactionTraceTabType(searchParams.get("tab"), supportsTraceActions))
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
    stateChangesRequestedHashRef.current = undefined
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
      setTraceOverview(undefined)
      setHoveredAction(undefined)
      try {
        const data = await client.getTraces(traceLookupHash, {
          includeActions: supportsTraceActions,
        })
        if (!isActive) return
        updateDomains(data.address_book)

        if (data.traces && data.traces.length > 0) {
          const trace = data.traces[0]
          const transactionsMap = trace.transactions
          const processed = buildTraceTransactionInfos(transactionsMap, trace.trace)
          const actions = trace.actions ?? []
          const enrichment = await enrichTraceTransactions({
            client,
            metadataRegistry,
            transactions: processed,
            transactionsMap,
            fetchName: fetchNameRef.current,
            addressFormat: addressFormatRef.current,
            actions,
            actionMetadata: data.metadata,
            shouldContinue: () => isActive,
          })
          if (!enrichment) {
            return
          }
          const nextTraceOverview = traceOverviewDataFromTrace(trace)
          if (isActive) {
            setTraces(enrichment.transactions)
            setContracts(enrichment.contracts)
            setCompilerAbisByCodeHash(enrichment.compilerAbisByCodeHash)
            setVerifiedSourcesByCodeHash(enrichment.verifiedSourcesByCodeHash)
            setValueFlow(enrichment.valueFlow)
            setTraceActions(actions)
            setTraceActionMetadata(data.metadata)
            setTraceOverview(nextTraceOverview)
          }
        } else if (isActive) setError("Transaction not found or has no trace yet.")
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
  }, [client, metadataRegistry, supportsTraceActions, traceLookupHash, updateDomains])

  useEffect(() => {
    const requestedHash = traceLookupHash.toLowerCase()
    if (
      activeTab !== "state-changes" ||
      traces.length === 0 ||
      stateChangesRequestedHashRef.current?.toLowerCase() === requestedHash
    ) {
      return
    }

    let isActive = true
    let completed = false
    stateChangesRequestedHashRef.current = requestedHash
    setStateChangesStatus({traceHash: requestedHash, isLoading: true})

    const loadTraceStateChanges = async () => {
      try {
        const {traceTransactionTree} = await import("../retrace/txTrace/lib/traceTx")
        const replayedTrace = await traceTransactionTree(traceLookupHash, network)
        if (!isActive) return

        if (!replayedTrace.result.stateUpdateHashOk) {
          const divergentTransactions = Object.values(replayedTrace.result.transactions).filter(
            result => !result.stateUpdateHashOk,
          ).length
          throw new Error(
            `State changes could not be verified for ${divergentTransactions} ${divergentTransactions === 1 ? "transaction" : "transactions"}`,
          )
        }

        setTraces(currentTraces =>
          withRetracedTraceStorage(currentTraces, replayedTrace.result.transactions),
        )
        setStateChangesStatus({traceHash: requestedHash, isLoading: false})
        completed = true
      } catch (error) {
        console.error("Failed to retrace transaction tree:", error)
        if (!isActive) return

        setStateChangesStatus({
          traceHash: requestedHash,
          isLoading: false,
          error: error instanceof Error ? error.message : "Failed to load decoded state changes",
        })
      }
    }

    void loadTraceStateChanges()
    return () => {
      isActive = false
      if (!completed && stateChangesRequestedHashRef.current === requestedHash) {
        stateChangesRequestedHashRef.current = undefined
      }
    }
  }, [activeTab, network, traceLookupHash, traces.length])

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

  const traceAddress = traces[0]?.address?.toString() ?? ""
  const traceAddressDisplay = normalizeAddress(traceAddress, addressFormat)
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
    <TransactionTraceView
      client={client}
      hash={hash}
      traces={traces}
      contracts={contracts}
      compilerAbisByCodeHash={compilerAbisByCodeHash}
      verifiedSourcesByCodeHash={verifiedSourcesByCodeHash}
      valueFlow={valueFlow}
      activeTab={activeTab}
      supportsTraceActions={supportsTraceActions}
      traceActions={traceActions}
      traceActionMetadata={traceActionMetadata}
      traceOverview={traceOverview}
      hoveredAction={hoveredAction}
      nowSeconds={nowSeconds}
      breadcrumbs={[
        {
          label: traceAddressDisplay,
          path: routes.addressPath(traceAddressDisplay),
          isAddress: true,
        },
        {
          label: hash,
          isHash: true,
          copy: {
            value: hash,
            label: "Copy transaction hash",
            copiedLabel: "Transaction hash copied",
          },
        },
      ]}
      isFavorite={favorite}
      stateChangesLoading={currentStateChangesStatus?.isLoading ?? false}
      stateChangesError={currentStateChangesStatus?.error}
      onTabChange={handleActiveTabChange}
      onActionHoverChange={setHoveredAction}
      onContractClick={handleContractClick}
      onTransactionSelect={handleTransactionSelect}
      onBlockClick={handleBlockClick}
      onToggleFavorite={selectedTraceTransaction ? handleToggleFavorite : undefined}
      loadActions={loadTransactionActions}
      resolveVerifiedSourceByCodeHash={resolveVerifiedSourceByCodeHash}
      renderSelectedTransactionExtra={renderSelectedTransactionExtra}
      renderSelectedTransactionMessageRouteAction={renderSelectedTransactionMessageRouteAction}
    />
  )
}

export interface TransactionTraceViewProps {
  readonly client?: TonClient
  readonly hash: string
  readonly traces: readonly TransactionInfo[]
  readonly contracts: Map<string, ContractData>
  readonly compilerAbisByCodeHash: ReadonlyMap<string, ContractData["abi"]>
  readonly verifiedSourcesByCodeHash: ReadonlyMap<string, ContractVerifiedSource>
  readonly valueFlow: readonly ValueFlowItem[]
  readonly activeTab: TransactionTraceTabType
  readonly supportsTraceActions?: boolean
  readonly traceActions?: readonly V3Action[]
  readonly traceActionMetadata?: V3Metadata
  readonly traceOverview?: TraceOverviewData
  readonly statusLabels?: {
    readonly success: string
    readonly error: string
  }
  readonly hoveredAction?: V3Action
  readonly nowSeconds?: number
  readonly breadcrumbs?: ComponentProps<typeof ExplorerBreadcrumbs>["items"]
  readonly isFavorite?: boolean
  readonly stateChangesLoading?: boolean
  readonly stateChangesError?: string
  readonly onTabChange: (tab: TransactionTraceTabType) => void
  readonly onActionHoverChange?: (action: V3Action | undefined) => void
  readonly onContractClick: (address: string, event?: ExplorerNavigationClickEvent) => void
  readonly onTransactionSelect?: (tx: TransactionInfo) => void
  readonly onBlockClick: (
    blockRef: TransactionBlockRef,
    event?: ExplorerNavigationClickEvent,
  ) => void
  readonly onToggleFavorite?: () => void
  readonly getBlockPath?: (blockRef: TransactionBlockRef) => string | undefined
  readonly loadActions?: (tx: TransactionInfo) => Promise<LoadedTransactionActions>
  readonly resolveVerifiedSourceByCodeHash?: ResolveVerifiedSourceByCodeHash
  readonly renderSelectedTransactionExtra?: (tx: TransactionInfo) => JSX.Element | null
  readonly renderSelectedTransactionMessageRouteAction?: (tx: TransactionInfo) => JSX.Element
}

export function TransactionTraceView({
  client,
  hash,
  traces,
  contracts,
  compilerAbisByCodeHash,
  verifiedSourcesByCodeHash,
  valueFlow,
  activeTab,
  supportsTraceActions = false,
  traceActions = [],
  traceActionMetadata = {},
  traceOverview,
  statusLabels = {
    success: "Confirmed transaction",
    error: "Failed transaction",
  },
  hoveredAction,
  nowSeconds = Math.floor(Date.now() / 1000),
  breadcrumbs,
  isFavorite = false,
  stateChangesLoading = false,
  stateChangesError,
  onTabChange,
  onActionHoverChange,
  onContractClick,
  onTransactionSelect,
  onBlockClick,
  onToggleFavorite,
  getBlockPath = blockPath,
  loadActions,
  resolveVerifiedSourceByCodeHash,
  renderSelectedTransactionExtra,
  renderSelectedTransactionMessageRouteAction,
}: TransactionTraceViewProps): JSX.Element {
  const {flowMetrics: treeFlowMetrics, rootRef: treeSectionRef} =
    useAvailableFlowMetrics<HTMLDivElement>(MAX_TRACE_TREE_FLOW_WIDTH)
  const firstTrace = traces[0]
  const firstTraceComputePhase = firstTrace
    ? getTransactionComputePhase(firstTrace.transaction)
    : undefined
  const firstTraceSucceeded =
    firstTraceComputePhase?.type === "vm" && firstTraceComputePhase.success
  const traceAddress = firstTrace?.address?.toString() ?? ""
  const rootTraceTransactions = [...traces]
    .filter(tx => !tx.parent)
    .sort(compareTransactionInfoByLt)
  const selectedTraceTransaction = traces.find(
    tx => transactionHashHex(tx).toLowerCase() === hash.toLowerCase(),
  )
  const selectedTransactionId = selectedTraceTransaction?.id
  const decodedMessageNamesByTransactionHash = useMemo(() => {
    const names = new Map<string, string>()
    for (const tx of traces) {
      const parsedBody = decodeTransactionMessageBody(tx, contracts, [], compilerAbisByCodeHash)
      if (parsedBody && parsedBody.opcode === undefined) {
        names.set(transactionHashHex(tx).toLowerCase(), parsedBody.name)
      }
    }
    return names
  }, [compilerAbisByCodeHash, contracts, traces])
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
  const renderTraceAddressChip = (
    address: string,
    options: {readonly shorten: boolean},
  ): JSX.Element => (
    <ExplorerAddressChip
      address={address}
      onAddressClick={onContractClick}
      resolveName={false}
      shorten={options.shorten}
    />
  )
  const showEventOverview = supportsTraceActions && client !== undefined

  return (
    <div className={styles.container}>
      <div className={styles.content}>
        {traces.length > 0 && firstTrace && (
          <>
            {breadcrumbs && <ExplorerBreadcrumbs items={breadcrumbs} />}
            <div className={styles.preTreeContent}>
              <div className={styles.overviewCard}>
                <div className={styles.overviewHeader}>
                  <div
                    className={`${styles.status} ${firstTraceSucceeded ? styles.statusSuccess : styles.statusError}`}
                  >
                    {firstTraceSucceeded ? (
                      <>
                        <CheckCircle2 size={18} /> {statusLabels.success}
                      </>
                    ) : (
                      <>
                        <XCircle size={18} /> {statusLabels.error}
                      </>
                    )}
                  </div>
                  <div className={styles.overviewMeta}>
                    <div className={styles.value}>
                      {new Date(firstTrace.transaction.now * 1000).toLocaleString()}
                    </div>
                    {onToggleFavorite && (
                      <InlineAction
                        className={isFavorite ? styles.favoriteActionActive : undefined}
                        label={isFavorite ? "Remove from favorites" : "Add to favorites"}
                        icon={
                          <Star className={isFavorite ? styles.favoriteIconActive : undefined} />
                        }
                        aria-pressed={isFavorite}
                        onClick={onToggleFavorite}
                      />
                    )}
                  </div>
                </div>
              </div>

              <div
                className={`${styles.tabsContainer} ${activeTab === "transactions" ? styles.tabsContainerDetached : ""}`}
              >
                <TraceTabs
                  activeTab={activeTab}
                  showEventOverview={showEventOverview}
                  onTabChange={onTabChange}
                />

                <div className={styles.tabContent}>
                  {activeTab === "value-flow" && (
                    <ValueFlowTable
                      items={valueFlow}
                      contracts={contracts}
                      onContractClick={onContractClick}
                      className={styles.valueFlowPanel}
                    />
                  )}

                  {activeTab === "event-overview" && showEventOverview && (
                    <ActionHistoryTable
                      actions={traceActions}
                      actionMetadata={traceActionMetadata}
                      decodedMessageNamesByTransactionHash={decodedMessageNamesByTransactionHash}
                      ownerAddress={traceAddress}
                      client={client}
                      nowSeconds={nowSeconds}
                      emptyState="No actions found"
                      showTimeColumn={false}
                      interactiveRows={false}
                      className={styles.valueFlowPanel}
                      onAddressClick={onContractClick}
                      onActionHoverChange={onActionHoverChange}
                    />
                  )}

                  {activeTab === "state-changes" && (
                    <TraceStateChangesPanel
                      transactions={traces}
                      contracts={contracts}
                      verifiedSourcesByCodeHash={verifiedSourcesByCodeHash}
                      resolveVerifiedSourceByCodeHash={resolveVerifiedSourceByCodeHash}
                      isLoading={stateChangesLoading}
                      error={stateChangesError}
                      onContractClick={onContractClick}
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
                          onContractClick={onContractClick}
                          getBlockPath={getBlockPath}
                          onBlockClick={onBlockClick}
                          loadActions={loadActions}
                          resolveVerifiedSourceByCodeHash={resolveVerifiedSourceByCodeHash}
                        />
                      ))}
                    </div>
                  )}

                  {activeTab === "details" && traceOverview && (
                    <TraceOverviewTable
                      data={traceOverview}
                      transactions={traces}
                      actionCount={supportsTraceActions ? traceActions.length : undefined}
                      onBlockClick={onBlockClick}
                    />
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
                transactions={[...traces]}
                contracts={contracts}
                compilerAbisByCodeHash={compilerAbisByCodeHash}
                verifiedSourcesByCodeHash={verifiedSourcesByCodeHash}
                allContracts={[]}
                selectedTransactionId={selectedTransactionId}
                highlightedTransactionIds={highlightedTransactionIds}
                onContractClick={onContractClick}
                onTransactionSelect={onTransactionSelect}
                renderAddressChip={renderTraceAddressChip}
                renderSelectedTransactionExtra={renderSelectedTransactionExtra}
                renderSelectedTransactionMessageRouteAction={
                  renderSelectedTransactionMessageRouteAction
                }
                getBlockPath={getBlockPath}
                onBlockClick={onBlockClick}
                loadActions={loadActions}
                resolveVerifiedSourceByCodeHash={resolveVerifiedSourceByCodeHash}
              />
            </div>
          </>
        )}
      </div>
    </div>
  )
}

interface TraceTabsProps {
  readonly activeTab: TransactionTraceTabType
  readonly disabled?: boolean
  readonly showEventOverview?: boolean
  readonly onTabChange?: (tab: TransactionTraceTabType) => void
}

function TraceTabs({
  activeTab,
  disabled = false,
  showEventOverview = false,
  onTabChange,
}: TraceTabsProps): JSX.Element {
  const activeTabRef = useRef<HTMLButtonElement>(null)

  useLayoutEffect(() => {
    if (disabled) return
    activeTabRef.current?.scrollIntoView({block: "nearest", inline: "nearest"})
  }, [activeTab, disabled])

  return (
    <div className={styles.tabs} aria-hidden={disabled ? "true" : undefined}>
      {showEventOverview && (
        <button
          type="button"
          ref={activeTab === "event-overview" ? activeTabRef : undefined}
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
        ref={activeTab === "value-flow" ? activeTabRef : undefined}
        className={`${styles.tab} ${activeTab === "value-flow" ? styles.tabActive : ""}`}
        onClick={() => onTabChange?.("value-flow")}
        disabled={disabled}
        tabIndex={disabled ? -1 : undefined}
      >
        <CircleDotDashed size={16} /> Value Flow
      </button>
      <button
        type="button"
        ref={activeTab === "transactions" ? activeTabRef : undefined}
        className={`${styles.tab} ${activeTab === "transactions" ? styles.tabActive : ""}`}
        onClick={() => onTabChange?.("transactions")}
        disabled={disabled}
        tabIndex={disabled ? -1 : undefined}
      >
        <GitBranch size={16} /> Transactions
      </button>
      <button
        type="button"
        ref={activeTab === "state-changes" ? activeTabRef : undefined}
        className={`${styles.tab} ${activeTab === "state-changes" ? styles.tabActive : ""}`}
        onClick={() => onTabChange?.("state-changes")}
        disabled={disabled}
        tabIndex={disabled ? -1 : undefined}
      >
        <Database size={16} /> State Changes
      </button>
      <button
        type="button"
        ref={activeTab === "details" ? activeTabRef : undefined}
        className={`${styles.tab} ${activeTab === "details" ? styles.tabActive : ""}`}
        onClick={() => onTabChange?.("details")}
        disabled={disabled}
        tabIndex={disabled ? -1 : undefined}
      >
        <Info size={16} /> Details
      </button>
    </div>
  )
}

interface TransactionTraceSkeletonProps {
  readonly activeTab: TransactionTraceTabType
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
              ) : activeTab === "state-changes" ? (
                <StateChangesSkeleton />
              ) : activeTab === "event-overview" ? (
                <ActionHistorySkeleton />
              ) : activeTab === "details" ? (
                <TraceOverviewSkeleton />
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

function TraceOverviewSkeleton(): JSX.Element {
  return (
    <div className={styles.skeletonOverviewTable} aria-hidden="true">
      {[0, 1, 2, 3, 4, 5].map(index => (
        <div key={`trace-overview-skeleton-${index}`} className={styles.skeletonOverviewRow}>
          <span className={styles.skeletonOverviewTitle}>
            <span className={`${styles.skeleton} ${styles.skeletonOverviewLabel}`} />
          </span>
          <span className={styles.skeletonOverviewContent}>
            {[0, 1, 2].map(itemIndex => (
              <span
                key={`trace-overview-skeleton-${index}-${itemIndex}`}
                className={`${styles.skeleton} ${styles.skeletonOverviewValue}`}
              />
            ))}
          </span>
        </div>
      ))}
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

function StateChangesSkeleton(): JSX.Element {
  return (
    <div className={styles.skeletonStateChanges} aria-hidden="true">
      {[0, 1].map(index => (
        <div key={`state-changes-skeleton-${index}`} className={styles.skeletonStateCard}>
          <div className={styles.skeletonStateHeader}>
            <span className={`${styles.skeleton} ${styles.skeletonStateAddress}`} />
          </div>
          <div className={styles.skeletonStateGrid}>
            {[0, 1, 2, 3].map(cellIndex => (
              <span
                key={`state-changes-skeleton-${index}-${cellIndex}`}
                className={`${styles.skeleton} ${styles.skeletonStateCell}`}
              />
            ))}
          </div>
          <span className={`${styles.skeleton} ${styles.skeletonStateStorage}`} />
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
  getBlockPath,
  onBlockClick,
  loadActions,
  resolveVerifiedSourceByCodeHash,
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
            getBlockPath={getBlockPath}
            onBlockClick={onBlockClick}
            loadActions={loadActions}
            resolveVerifiedSourceByCodeHash={resolveVerifiedSourceByCodeHash}
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
              getBlockPath={getBlockPath}
              onBlockClick={onBlockClick}
              loadActions={loadActions}
              resolveVerifiedSourceByCodeHash={resolveVerifiedSourceByCodeHash}
            />
          ))}
        </div>
      )}
    </div>
  )
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

import {type FormEvent, useCallback, useEffect, useMemo, useRef, useState} from "react"
import {Bug, FileJson, Play, Plus, RefreshCw, RotateCcw, Trash2, WandSparkles} from "lucide-react"
import {
  Button,
  Checkbox,
  ContentTabs,
  InlineButton,
  Input,
  RawDataBlock,
  Select,
  useToast,
} from "@acton/ui"
import {
  AbiValueEditor,
  buildAbiMessageBoc,
  buildAbiStorageDataBoc,
  createAbiMessageSymbols,
  createAbiStorageSymbols,
  decodeAbiStorageDataBoc,
  formatAbiMessageOptionSummary,
  getAbiStorageBuilderInfo,
  listAbiMessageBuilderOptions,
  parseAbiCellArg,
  parseAbiJson,
  stringifyAbiJson,
  type AbiMessageBuilderOption,
  type AbiMessageTransport,
  type LoadedTransactionActions,
  type TransactionBlockRef,
  type TransactionInfo,
} from "@acton/transaction-ui"
import {useNavigate, useSearchParams} from "react-router-dom"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {Address, Cell, fromNano, loadShardAccount, toNano, type ShardAccount} from "@ton/core"

import {useNetworkInfo} from "../hooks/useNetworkInfo"
import {useAddressFormat} from "../hooks/useNetworkInfo"
import {useAddressBook} from "../hooks/useAddressBook"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {openExplorerPath, type ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"
import {normalizeAddress} from "../components/utils"
import type {TonClient} from "../api/client"
import {addressKey} from "../api/compilerAbi"
import {resolveCompilerAbis} from "../api/compilerAbiResolver"
import {ExplorerBreadcrumbs} from "../components/ExplorerBreadcrumbs"
import {useMetadataRegistry} from "../metadata/MetadataRegistryProvider"
import {
  emulateRawMessageBoc,
  parseRawMessageBoc,
  type RawMessageEmulationOptions,
  type RawMessageEmulationResult,
} from "../retrace/txTrace/lib/emulateRawMessage"
import RetraceWorkspace from "../retrace/txTrace/ui/RetraceWorkspace"
import {TraceDebugPanel} from "../retrace/txTrace/ui/TraceDebugPanel/TraceDebugPanel"
import "../retrace/Retrace.tokens.css"
import {
  TransactionTraceView,
  type TransactionTraceTabType,
  traceOverviewDataFromTrace,
  transactionHashHex,
} from "./TransactionPage"
import {
  enrichTraceTransactions,
  type TraceTransactionEnrichmentResult,
} from "./transactionTraceEnrichment"
import styles from "./EmulatePage.module.css"

type EmulateInputMode = "builder" | "raw"
type AbiSourceMode = "auto" | "manual"
type AbiValueInputMode = "form" | "json"
type AccountOverrideMode = "fields" | "shardAccount"
type AccountStateOverrideKind = "keep" | "active" | "uninit" | "frozen"
type StorageOverrideSource = "abi" | "raw"
const EMULATE_ADDRESS_QUERY_PARAM = "address"
const EMULATE_SOURCE_QUERY_PARAM = "source"
const EMULATE_VALUE_QUERY_PARAM = "value"
const DEFAULT_MESSAGE_VALUE = "0.5"

interface EmulateSearchFields {
  readonly targetAddress: string
  readonly sourceAddress: string
  readonly messageValue: string
}

function readEmulateSearchFields(searchParams: URLSearchParams): EmulateSearchFields {
  return {
    targetAddress: searchParams.get(EMULATE_ADDRESS_QUERY_PARAM) ?? "",
    sourceAddress: searchParams.get(EMULATE_SOURCE_QUERY_PARAM) ?? "",
    messageValue: searchParams.get(EMULATE_VALUE_QUERY_PARAM) ?? DEFAULT_MESSAGE_VALUE,
  }
}

function areEmulateSearchFieldsEqual(
  left: EmulateSearchFields,
  right: EmulateSearchFields,
): boolean {
  return (
    left.targetAddress === right.targetAddress &&
    left.sourceAddress === right.sourceAddress &&
    left.messageValue === right.messageValue
  )
}
type AccountOverrideLoadState =
  | {readonly type: "idle"}
  | {readonly type: "loading"}
  | {readonly type: "ready"; readonly message?: string}
  | {readonly type: "error"}
type AbiLoadState =
  | {readonly type: "idle"}
  | {readonly type: "loading"}
  | {readonly type: "ready"; readonly label: string}
  | {readonly type: "error"; readonly message: string}

type EmulateState =
  | {readonly type: "idle"}
  | {readonly type: "loading"}
  | {
      readonly type: "ready"
      readonly result: RawMessageEmulationResult
      readonly enrichment: TraceTransactionEnrichmentResult
      readonly mcSeqno?: number
    }
  | {readonly type: "error"; readonly message: string}

interface EmulatePageProps {
  readonly client: TonClient
}

interface AccountStateOverrideDraft {
  readonly id: string
  readonly address: string
  readonly abi?: ContractABI
  readonly loadedAddress?: string
  readonly loadState: AccountOverrideLoadState
  readonly mode: AccountOverrideMode
  readonly shardAccountBoc: string
  readonly balance: string
  readonly stateKind: AccountStateOverrideKind
  readonly codeBoc: string
  readonly storageEnabled: boolean
  readonly storageSource: StorageOverrideSource
  readonly storageInputMode: AbiValueInputMode
  readonly storageJson: string
  readonly storageFormValue: unknown
  readonly dataBoc: string
  readonly frozenHash: string
  readonly lastTransactionLt: string
  readonly lastTransactionHash: string
}

export function EmulatePage({client}: EmulatePageProps) {
  const {network} = useNetworkInfo()
  const addressFormat = useAddressFormat()
  const {fetchName} = useAddressBook()
  const metadataRegistry = useMetadataRegistry()
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  const routes = useExplorerRoutePaths()
  const {showToast} = useToast()
  const [inputMode, setInputMode] = useState<EmulateInputMode>("builder")
  const [targetAddress, setTargetAddress] = useState(
    () => readEmulateSearchFields(searchParams).targetAddress,
  )
  const [sourceAddress, setSourceAddress] = useState(
    () => readEmulateSearchFields(searchParams).sourceAddress,
  )
  const [messageValue, setMessageValue] = useState(
    () => readEmulateSearchFields(searchParams).messageValue,
  )
  const [messageTransport, setMessageTransport] = useState<AbiMessageTransport>("internal")
  const [bounce, setBounce] = useState(true)
  const [abiSourceMode, setAbiSourceMode] = useState<AbiSourceMode>("auto")
  const [manualAbiJson, setManualAbiJson] = useState("")
  const [loadedAbi, setLoadedAbi] = useState<ContractABI | undefined>()
  const [abiLoadState, setAbiLoadState] = useState<AbiLoadState>({type: "idle"})
  const [selectedMessageId, setSelectedMessageId] = useState("")
  const [argsInputMode, setArgsInputMode] = useState<AbiValueInputMode>("form")
  const [argsJson, setArgsJson] = useState("{}")
  const [argsFormValue, setArgsFormValue] = useState<unknown>({})
  const [rawMessage, setRawMessage] = useState("")
  const [mcSeqnoInput, setMcSeqnoInput] = useState("")
  const [ignoreChksig, setIgnoreChksig] = useState(false)
  const [stateOverrideEnabled, setStateOverrideEnabled] = useState(false)
  const [stateOverrideEntries, setStateOverrideEntries] = useState<
    readonly AccountStateOverrideDraft[]
  >([])
  const [state, setState] = useState<EmulateState>({type: "idle"})
  const [activeTab, setActiveTab] = useState<TransactionTraceTabType>("value-flow")
  const [selectedHash, setSelectedHash] = useState<string | undefined>()
  const [expandedDebugHash, setExpandedDebugHash] = useState<string | undefined>()
  const latestAbiLoadRequest = useRef(0)
  const nextStateOverrideId = useRef(1)
  const lastSearchFields = useRef(readEmulateSearchFields(searchParams))
  const isApplyingSearchFields = useRef(false)

  const isLoading = state.type === "loading"
  const emulation = state.type === "ready" ? state.result : undefined
  const enrichment = state.type === "ready" ? state.enrichment : undefined
  const selectedTraceHash = selectedHash ?? emulation?.result.rootTxHash ?? ""
  const contracts = useMemo(() => enrichment?.contracts ?? new Map(), [enrichment])
  const compilerAbisByCodeHash = useMemo(
    () => enrichment?.compilerAbisByCodeHash ?? new Map(),
    [enrichment],
  )
  const verifiedSourcesByCodeHash = useMemo(
    () => enrichment?.verifiedSourcesByCodeHash ?? new Map(),
    [enrichment],
  )
  const valueFlow = enrichment?.valueFlow ?? []
  const manualAbi = useMemo(() => parseManualAbi(manualAbiJson), [manualAbiJson])
  const activeAbi = abiSourceMode === "manual" ? manualAbi.abi : loadedAbi
  const abiParseError = abiSourceMode === "manual" ? manualAbi.error : undefined
  const messageSymbols = useMemo(
    () => (activeAbi ? createAbiMessageSymbols(activeAbi) : undefined),
    [activeAbi],
  )
  const builderOptions = useMemo(
    () => (activeAbi ? listAbiMessageBuilderOptions(activeAbi, messageTransport) : []),
    [activeAbi, messageTransport],
  )
  const selectedBuilderOption = useMemo(
    () => builderOptions.find(option => option.id === selectedMessageId),
    [builderOptions, selectedMessageId],
  )
  const builderPreview = useMemo(
    () =>
      buildBuilderPreview({
        abi: activeAbi,
        option: selectedBuilderOption,
        destination: targetAddress,
        source: sourceAddress,
        value: messageValue,
        bounce,
        argsJson,
      }),
    [
      activeAbi,
      argsJson,
      bounce,
      messageValue,
      selectedBuilderOption,
      sourceAddress,
      targetAddress,
    ],
  )
  const activeRawMessage = inputMode === "builder" ? builderPreview.boc : rawMessage
  const stateOverrideAbiContexts = useMemo(
    () =>
      new Map(
        stateOverrideEntries.map(entry => {
          const storageInfo = getAbiStorageBuilderInfo(entry.abi)
          return [
            entry.id,
            {
              abi: entry.abi,
              storageInfo,
              storageSymbols:
                entry.abi && storageInfo ? createAbiStorageSymbols(entry.abi) : undefined,
            },
          ] as const
        }),
      ),
    [stateOverrideEntries],
  )
  const stateOverrideStoragePreviews = useMemo(
    () =>
      new Map(
        stateOverrideEntries.map(entry => [
          entry.id,
          buildStorageOverridePreview({
            enabled: stateOverrideEnabled && entry.mode === "fields" && entry.storageEnabled,
            source: entry.storageSource,
            abi: stateOverrideAbiContexts.get(entry.id)?.abi,
            storageJson: entry.storageJson,
            rawDataBoc: entry.dataBoc,
          }),
        ]),
      ),
    [stateOverrideAbiContexts, stateOverrideEnabled, stateOverrideEntries],
  )
  const stateOverrideAccountCount = stateOverrideEntries.length
  const canApplyStateOverride =
    !stateOverrideEnabled ||
    (stateOverrideEntries.length > 0 &&
      stateOverrideEntries.every(entry => {
        const preview = stateOverrideStoragePreviews.get(entry.id)
        return (
          Boolean(entry.address.trim()) &&
          entry.loadState.type !== "loading" &&
          (entry.mode === "fields" || Boolean(entry.shardAccountBoc.trim())) &&
          preview?.error === undefined
        )
      }))
  const canEmulate =
    canApplyStateOverride &&
    (inputMode === "builder"
      ? Boolean(builderPreview.boc) && builderPreview.error === undefined
      : Boolean(rawMessage.trim()))

  useEffect(() => {
    const fieldsFromUrl = readEmulateSearchFields(searchParams)
    if (areEmulateSearchFieldsEqual(fieldsFromUrl, lastSearchFields.current)) {
      return
    }

    lastSearchFields.current = fieldsFromUrl
    isApplyingSearchFields.current = true
    setTargetAddress(fieldsFromUrl.targetAddress)
    setSourceAddress(fieldsFromUrl.sourceAddress)
    setMessageValue(fieldsFromUrl.messageValue)
  }, [searchParams])

  useEffect(() => {
    if (isApplyingSearchFields.current) {
      isApplyingSearchFields.current = false
      return
    }

    const nextFields = {
      targetAddress: targetAddress.trim(),
      sourceAddress: sourceAddress.trim(),
      messageValue: messageValue.trim() || DEFAULT_MESSAGE_VALUE,
    }
    if (areEmulateSearchFieldsEqual(readEmulateSearchFields(searchParams), nextFields)) {
      return
    }

    const nextParams = new URLSearchParams(searchParams)
    if (nextFields.targetAddress) {
      nextParams.set(EMULATE_ADDRESS_QUERY_PARAM, nextFields.targetAddress)
    } else {
      nextParams.delete(EMULATE_ADDRESS_QUERY_PARAM)
    }
    if (nextFields.sourceAddress) {
      nextParams.set(EMULATE_SOURCE_QUERY_PARAM, nextFields.sourceAddress)
    } else {
      nextParams.delete(EMULATE_SOURCE_QUERY_PARAM)
    }
    if (nextFields.messageValue === DEFAULT_MESSAGE_VALUE) {
      nextParams.delete(EMULATE_VALUE_QUERY_PARAM)
    } else {
      nextParams.set(EMULATE_VALUE_QUERY_PARAM, nextFields.messageValue)
    }
    lastSearchFields.current = nextFields
    setSearchParams(nextParams, {replace: true})
  }, [messageValue, searchParams, setSearchParams, sourceAddress, targetAddress])

  const createStateOverrideDraft = useCallback(
    (address = "") =>
      createAccountStateOverrideDraft({
        id: `override-${nextStateOverrideId.current++}`,
        address,
      }),
    [],
  )

  const updateStateOverrideEntry = useCallback(
    (entryId: string, updater: (entry: AccountStateOverrideDraft) => AccountStateOverrideDraft) => {
      setStateOverrideEntries(entries =>
        entries.map(entry => (entry.id === entryId ? updater(entry) : entry)),
      )
    },
    [],
  )

  const handleStateOverrideEnabledChange = useCallback(
    (checked: boolean) => {
      setStateOverrideEnabled(checked)
      if (checked) {
        setStateOverrideEntries(entries =>
          entries.length > 0 ? entries : [createStateOverrideDraft(targetAddress.trim())],
        )
      }
    },
    [createStateOverrideDraft, targetAddress],
  )

  const handleAddStateOverrideEntry = useCallback(() => {
    setStateOverrideEntries(entries => [...entries, createStateOverrideDraft()])
  }, [createStateOverrideDraft])

  const handleRemoveStateOverrideEntry = useCallback((entryId: string) => {
    setStateOverrideEntries(entries => entries.filter(entry => entry.id !== entryId))
  }, [])

  const preloadStateOverrideEntry = useCallback(
    async (entryId: string, addressInput: string) => {
      const address = addressInput.trim()
      if (!address || !isValidAddress(address)) {
        return
      }

      setStateOverrideEntries(entries =>
        entries.map(entry =>
          entry.id === entryId ? {...entry, loadState: {type: "loading"}} : entry,
        ),
      )

      try {
        const mcSeqno = parseMcSeqno(mcSeqnoInput)
        const shardAccountBoc = await client.getShardAccountCell(address, mcSeqno)
        const shardAccountCell = Cell.fromBase64(shardAccountBoc)
        const shardAccount = loadShardAccount(shardAccountCell.asSlice())
        const accountState = shardAccount.account?.storage.state
        const codeHash =
          accountState?.type === "active" && accountState.state.code
            ? accountState.state.code.hash().toString("hex")
            : undefined
        let accountAbi: ContractABI | undefined
        if (codeHash) {
          const compilerAbis = await metadataRegistry
            .getCompilerAbis([codeHash])
            .catch(() => ({[codeHash]: null}))
          accountAbi = compilerAbis[codeHash]?.compiler_abi
        }

        setStateOverrideEntries(entries =>
          entries.map(entry =>
            entry.id === entryId && entry.address.trim() === address
              ? hydrateAccountStateOverrideDraft(
                  entry,
                  address,
                  shardAccount,
                  shardAccountCell,
                  accountAbi,
                )
              : entry,
          ),
        )
      } catch (error) {
        const message = error instanceof Error ? error.message : "Failed to load account state"
        setStateOverrideEntries(entries =>
          entries.map(entry =>
            entry.id === entryId && entry.address.trim() === address
              ? {...entry, loadState: {type: "error"}}
              : entry,
          ),
        )
        showToast({
          title: "Failed to load account state",
          description: message,
          variant: "error",
        })
      }
    },
    [client, mcSeqnoInput, metadataRegistry, showToast],
  )

  useEffect(() => {
    if (!stateOverrideEnabled) {
      return
    }

    for (const entry of stateOverrideEntries) {
      const address = entry.address.trim()
      if (
        entry.loadState.type === "idle" &&
        entry.loadedAddress !== address &&
        isValidAddress(address)
      ) {
        void preloadStateOverrideEntry(entry.id, address)
      }
    }
  }, [preloadStateOverrideEntry, stateOverrideEnabled, stateOverrideEntries])

  useEffect(() => {
    if (builderOptions.length === 0) {
      if (selectedMessageId) {
        setSelectedMessageId("")
      }
      return
    }

    if (!builderOptions.some(option => option.id === selectedMessageId)) {
      const firstOption = builderOptions[0]
      setSelectedMessageId(firstOption.id)
      setArgsJson(firstOption.sampleJson)
      setArgsFormValue(parseAbiJson(firstOption.sampleJson, {}))
    }
  }, [builderOptions, selectedMessageId])

  useEffect(() => {
    const address = targetAddress.trim()
    if (!address) {
      setLoadedAbi(undefined)
      setAbiLoadState({type: "idle"})
      return
    }

    try {
      Address.parse(address)
    } catch {
      setLoadedAbi(undefined)
      setAbiLoadState({type: "idle"})
      return
    }

    const requestId = latestAbiLoadRequest.current + 1
    latestAbiLoadRequest.current = requestId
    setAbiLoadState({type: "loading"})

    const timeoutId = globalThis.setTimeout(() => {
      void resolveCompilerAbis({
        client,
        metadataRegistry,
        addresses: [address],
      })
        .then(resolved => {
          if (latestAbiLoadRequest.current !== requestId) {
            return
          }
          const abi = resolved?.abiByAddress.get(addressKey(address))
          if (!abi) {
            throw new Error("No ABI found for target contract.")
          }
          setLoadedAbi(abi)
          setAbiSourceMode("auto")
          setAbiLoadState({type: "ready", label: abi.contract_name})
        })
        .catch(error => {
          if (latestAbiLoadRequest.current !== requestId) {
            return
          }
          const message = error instanceof Error ? error.message : "Failed to load ABI"
          setLoadedAbi(undefined)
          setAbiLoadState({type: "error", message})
        })
    }, 350)

    return () => {
      globalThis.clearTimeout(timeoutId)
    }
  }, [client, metadataRegistry, targetAddress])

  const handleContractClick = useCallback(
    (address: string, event?: ExplorerNavigationClickEvent) => {
      const formattedAddress = normalizeAddress(address, addressFormat)
      openExplorerPath(navigate, routes.addressPath(formattedAddress), event)
    },
    [addressFormat, navigate, routes],
  )

  const handleBlockClick = useCallback(
    (blockRef: TransactionBlockRef, event?: ExplorerNavigationClickEvent) => {
      openExplorerPath(
        navigate,
        routes.blockPath(blockRef.workchain, blockRef.shard, blockRef.seqno),
        event,
      )
    },
    [navigate, routes],
  )

  const loadEmulatedActions = useCallback(
    async (tx: TransactionInfo): Promise<LoadedTransactionActions> => {
      const traceResult = emulation?.retraceResultsByHash.get(transactionHashHex(tx).toLowerCase())

      return {
        actions: traceResult?.result.emulatedTx.c5,
        outActions: traceResult?.result.emulatedTx.actions ?? [],
        executorActions: tx.executorActions,
      }
    },
    [emulation],
  )

  const handleLoadAbi = useCallback(async () => {
    const address = targetAddress.trim()
    if (!address) {
      const message = "Target contract address is required."
      setAbiLoadState({type: "error", message})
      showToast({title: "Failed to load ABI", description: message, variant: "error"})
      return
    }

    try {
      setAbiLoadState({type: "loading"})
      const resolved = await resolveCompilerAbis({
        client,
        metadataRegistry,
        addresses: [address],
      })
      const abi = resolved?.abiByAddress.get(addressKey(address))
      if (!abi) {
        throw new Error("No ABI found for target contract.")
      }

      setLoadedAbi(abi)
      setAbiSourceMode("auto")
      setAbiLoadState({type: "ready", label: abi.contract_name})
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to load ABI"
      setLoadedAbi(undefined)
      setAbiLoadState({type: "error", message})
      showToast({title: "Failed to load ABI", description: message, variant: "error"})
    }
  }, [client, metadataRegistry, showToast, targetAddress])

  const handleMessageOptionChange = useCallback(
    (messageId: string) => {
      const option = builderOptions.find(option => option.id === messageId)
      setSelectedMessageId(messageId)
      if (option) {
        setArgsJson(option.sampleJson)
        setArgsFormValue(parseAbiJson(option.sampleJson, {}))
      }
    },
    [builderOptions],
  )

  const handleArgsInputModeChange = useCallback(
    (mode: AbiValueInputMode) => {
      setArgsInputMode(mode)
      if (mode === "form") {
        setArgsFormValue(parseAbiJson(argsJson, argsFormValue))
      } else {
        setArgsJson(stringifyAbiJson(argsFormValue))
      }
    },
    [argsFormValue, argsJson],
  )

  const handleArgsFormChange = useCallback((value: unknown) => {
    setArgsFormValue(value)
    setArgsJson(stringifyAbiJson(value))
  }, [])

  const handleStorageInputModeChange = useCallback(
    (entryId: string, mode: AbiValueInputMode) => {
      updateStateOverrideEntry(entryId, entry => {
        if (mode === "form") {
          return {
            ...entry,
            storageInputMode: mode,
            storageFormValue: parseAbiJson(entry.storageJson, entry.storageFormValue),
          }
        }
        return {
          ...entry,
          storageInputMode: mode,
          storageJson: stringifyAbiJson(entry.storageFormValue),
        }
      })
    },
    [updateStateOverrideEntry],
  )

  const handleStorageFormChange = useCallback(
    (entryId: string, value: unknown) => {
      updateStateOverrideEntry(entryId, entry => ({
        ...entry,
        storageFormValue: value,
        storageJson: stringifyAbiJson(value),
      }))
    },
    [updateStateOverrideEntry],
  )

  const renderDebugAction = useCallback(
    (tx: TransactionInfo) => {
      const txHash = transactionHashHex(tx)
      const isOpen = expandedDebugHash === txHash

      return (
        <InlineButton
          type="button"
          variant="accent"
          className={isOpen ? styles.debugInlineButtonActive : undefined}
          leadingIcon={<Bug size={14} />}
          onClick={() => setExpandedDebugHash(isOpen ? undefined : txHash)}
          aria-expanded={isOpen}
        >
          Debug
        </InlineButton>
      )
    },
    [expandedDebugHash],
  )

  const renderDebugPanel = useCallback(
    (tx: TransactionInfo) => {
      const txHash = transactionHashHex(tx)
      if (expandedDebugHash !== txHash || !emulation) {
        return null
      }

      const traceResult = emulation.retraceResultsByHash.get(txHash.toLowerCase())
      if (!traceResult) {
        return null
      }

      return (
        <TraceDebugPanel onClose={() => setExpandedDebugHash(undefined)}>
          <RetraceWorkspace
            result={traceResult}
            contractAbi={tx.contractAbi}
            contracts={contracts}
            onContractClick={handleContractClick}
          />
        </TraceDebugPanel>
      )
    },
    [contracts, emulation, expandedDebugHash, handleContractClick],
  )

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()

    try {
      if (!activeRawMessage) {
        throw new Error("Message BOC is required")
      }
      parseRawMessageBoc(activeRawMessage)
      const mcSeqno = parseMcSeqno(mcSeqnoInput)
      const accountStateOverrides = buildAccountStateOverrides({
        enabled: stateOverrideEnabled,
        entries: stateOverrideEntries,
        storagePreviews: stateOverrideStoragePreviews,
      })

      setState({type: "loading"})
      const result = await emulateRawMessageBoc(activeRawMessage, network, {
        accountStateOverrides,
        ignoreChksig,
        mcSeqno,
      })
      const enrichment = await enrichTraceTransactions({
        client,
        metadataRegistry,
        transactions: result.transactions,
        transactionsMap: result.trace.transactions,
        fetchName,
        addressFormat,
      })
      if (!enrichment) {
        throw new Error("Failed to enrich emulated trace")
      }
      setSelectedHash(result.result.rootTxHash)
      setExpandedDebugHash(undefined)
      setActiveTab("value-flow")
      setState({type: "ready", result, enrichment, mcSeqno})
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to emulate message"
      setState({type: "error", message})
      showToast({
        title: "Failed to emulate message",
        description: message,
        variant: "error",
      })
    }
  }

  const handleReset = () => {
    setInputMode("builder")
    setTargetAddress("")
    setSourceAddress("")
    setMessageValue(DEFAULT_MESSAGE_VALUE)
    setMessageTransport("internal")
    setBounce(true)
    setAbiSourceMode("auto")
    setManualAbiJson("")
    setLoadedAbi(undefined)
    setAbiLoadState({type: "idle"})
    setSelectedMessageId("")
    setArgsInputMode("form")
    setArgsJson("{}")
    setArgsFormValue({})
    setRawMessage("")
    setMcSeqnoInput("")
    setIgnoreChksig(false)
    setStateOverrideEnabled(false)
    setStateOverrideEntries([])
    setSelectedHash(undefined)
    setExpandedDebugHash(undefined)
    setActiveTab("value-flow")
    setState({type: "idle"})
  }

  const stateOverrideControls = (
    <div className={styles.stateOverride}>
      <div className={styles.stateOverrideHeader}>
        <Checkbox
          checked={stateOverrideEnabled}
          onChange={event => handleStateOverrideEnabledChange(event.target.checked)}
          disabled={isLoading}
          label="Override state"
          className={styles.stateOverrideCheckbox}
        />
        {stateOverrideEnabled && (
          <span className={styles.panelMeta}>
            {stateOverrideAccountCount === 1
              ? "1 account"
              : `${stateOverrideAccountCount} accounts`}
          </span>
        )}
      </div>

      {stateOverrideEnabled && (
        <div className={styles.stateOverrideBody}>
          {stateOverrideEntries.map((entry, index) => {
            const storagePreview = stateOverrideStoragePreviews.get(entry.id) ?? {}
            const storageContext = stateOverrideAbiContexts.get(entry.id)
            const storageInfo = storageContext?.storageInfo
            const storageSymbols = storageContext?.storageSymbols
            const canLoadCurrent = isValidAddress(entry.address.trim())

            return (
              <section className={styles.stateOverrideAccount} key={entry.id}>
                <div className={styles.accountOverrideHeader}>
                  <div className={styles.accountOverrideTitle}>Account {index + 1}</div>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    leadingIcon={<Trash2 size={14} />}
                    onClick={() => handleRemoveStateOverrideEntry(entry.id)}
                    disabled={isLoading}
                  >
                    Remove
                  </Button>
                </div>

                <Input
                  fieldClassName={styles.field}
                  className={styles.addressInput}
                  label="Account address"
                  value={entry.address}
                  onChange={event =>
                    updateStateOverrideEntry(entry.id, current => ({
                      ...current,
                      address: event.target.value,
                      abi:
                        current.loadedAddress === event.target.value.trim()
                          ? current.abi
                          : undefined,
                      loadedAddress:
                        current.loadedAddress === event.target.value.trim()
                          ? current.loadedAddress
                          : undefined,
                      loadState: {type: "idle"},
                    }))
                  }
                  placeholder={targetAddress.trim() ? "Target contract" : "EQ..."}
                  disabled={isLoading}
                />

                <div className={styles.overrideFormatBlock}>
                  <span className={styles.fieldLabel}>Override format</span>
                  <div className={styles.overrideFormatRow}>
                    <div className={styles.segmentedControl} aria-label="State override mode">
                      <button
                        type="button"
                        className={`${styles.segment} ${
                          entry.mode === "fields" ? styles.segmentActive : ""
                        }`}
                        onClick={() =>
                          updateStateOverrideEntry(entry.id, current => ({
                            ...current,
                            mode: "fields",
                          }))
                        }
                        aria-pressed={entry.mode === "fields"}
                        disabled={isLoading}
                      >
                        Fields
                      </button>
                      <button
                        type="button"
                        className={`${styles.segment} ${
                          entry.mode === "shardAccount" ? styles.segmentActive : ""
                        }`}
                        onClick={() =>
                          updateStateOverrideEntry(entry.id, current => ({
                            ...current,
                            mode: "shardAccount",
                          }))
                        }
                        aria-pressed={entry.mode === "shardAccount"}
                        disabled={isLoading}
                      >
                        ShardAccount
                      </button>
                    </div>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      leadingIcon={<RefreshCw size={14} />}
                      loading={entry.loadState.type === "loading"}
                      onClick={() => void preloadStateOverrideEntry(entry.id, entry.address)}
                      disabled={isLoading || !canLoadCurrent}
                    >
                      Load current
                    </Button>
                  </div>
                </div>

                {entry.mode === "shardAccount" ? (
                  <label className={styles.field}>
                    <span className={styles.fieldLabel}>ShardAccount BOC</span>
                    <textarea
                      className={styles.stateOverrideBoc}
                      value={entry.shardAccountBoc}
                      onChange={event =>
                        updateStateOverrideEntry(entry.id, current => ({
                          ...current,
                          shardAccountBoc: event.target.value,
                        }))
                      }
                      placeholder="Hex ShardAccount BoC"
                      spellCheck={false}
                      disabled={isLoading}
                      rows={5}
                    />
                  </label>
                ) : (
                  <div className={styles.stateOverrideFields}>
                    <Input
                      fieldClassName={styles.field}
                      label="Balance"
                      suffix="GRAM"
                      value={entry.balance}
                      onChange={event =>
                        updateStateOverrideEntry(entry.id, current => ({
                          ...current,
                          balance: event.target.value,
                        }))
                      }
                      inputMode="decimal"
                      placeholder="0.5"
                      disabled={isLoading}
                    />

                    <Select
                      fieldClassName={styles.field}
                      label="Account state"
                      value={entry.stateKind}
                      onChange={event =>
                        updateStateOverrideEntry(entry.id, current => ({
                          ...current,
                          stateKind: event.target.value as AccountStateOverrideKind,
                        }))
                      }
                      disabled={isLoading}
                    >
                      <option value="keep">Keep current</option>
                      <option value="active">Active</option>
                      <option value="uninit">Uninit</option>
                      <option value="frozen">Frozen</option>
                    </Select>

                    {(entry.stateKind === "keep" || entry.stateKind === "active") && (
                      <div className={styles.stateOverrideNested}>
                        <label className={styles.field}>
                          <span className={styles.fieldLabel}>Code BOC</span>
                          <textarea
                            className={styles.stateOverrideBoc}
                            value={entry.codeBoc}
                            onChange={event =>
                              updateStateOverrideEntry(entry.id, current => ({
                                ...current,
                                codeBoc: event.target.value,
                              }))
                            }
                            placeholder="current code BOC"
                            spellCheck={false}
                            disabled={isLoading}
                            rows={3}
                          />
                        </label>

                        <div className={styles.stateOverrideStorageHeader}>
                          <Checkbox
                            checked={entry.storageEnabled}
                            onChange={event =>
                              updateStateOverrideEntry(entry.id, current => ({
                                ...current,
                                storageEnabled: event.target.checked,
                              }))
                            }
                            disabled={isLoading}
                            label="Override storage data"
                            className={styles.stateOverrideCheckbox}
                          />
                        </div>

                        {entry.storageEnabled && (
                          <div className={styles.stateOverrideStorage}>
                            <div className={styles.sourceSwitch} aria-label="Storage data source">
                              <button
                                type="button"
                                className={`${styles.sourceSwitchButton} ${
                                  entry.storageSource === "abi"
                                    ? styles.sourceSwitchButtonActive
                                    : ""
                                }`}
                                onClick={() =>
                                  updateStateOverrideEntry(entry.id, current => ({
                                    ...current,
                                    storageSource: "abi",
                                  }))
                                }
                                aria-pressed={entry.storageSource === "abi"}
                                disabled={isLoading}
                              >
                                ABI
                              </button>
                              <button
                                type="button"
                                className={`${styles.sourceSwitchButton} ${
                                  entry.storageSource === "raw"
                                    ? styles.sourceSwitchButtonActive
                                    : ""
                                }`}
                                onClick={() =>
                                  updateStateOverrideEntry(entry.id, current => ({
                                    ...current,
                                    storageSource: "raw",
                                  }))
                                }
                                aria-pressed={entry.storageSource === "raw"}
                                disabled={isLoading}
                              >
                                Raw
                              </button>
                            </div>

                            {entry.storageSource === "abi" ? (
                              <>
                                <div
                                  className={styles.sourceSwitch}
                                  aria-label="Storage input mode"
                                >
                                  <button
                                    type="button"
                                    className={`${styles.sourceSwitchButton} ${
                                      entry.storageInputMode === "form"
                                        ? styles.sourceSwitchButtonActive
                                        : ""
                                    }`}
                                    onClick={() => handleStorageInputModeChange(entry.id, "form")}
                                    aria-pressed={entry.storageInputMode === "form"}
                                    disabled={isLoading || !storageInfo || !storageSymbols}
                                  >
                                    Form
                                  </button>
                                  <button
                                    type="button"
                                    className={`${styles.sourceSwitchButton} ${
                                      entry.storageInputMode === "json"
                                        ? styles.sourceSwitchButtonActive
                                        : ""
                                    }`}
                                    onClick={() => handleStorageInputModeChange(entry.id, "json")}
                                    aria-pressed={entry.storageInputMode === "json"}
                                    disabled={isLoading || !storageInfo}
                                  >
                                    JSON
                                  </button>
                                </div>

                                {storageInfo && storageSymbols ? (
                                  entry.storageInputMode === "form" ? (
                                    <AbiValueEditor
                                      symbols={storageSymbols}
                                      tyIdx={storageInfo.tyIdx}
                                      value={entry.storageFormValue}
                                      onChange={value => handleStorageFormChange(entry.id, value)}
                                      disabled={isLoading}
                                    />
                                  ) : (
                                    <label className={styles.field}>
                                      <span className={styles.fieldLabel}>Storage JSON</span>
                                      <textarea
                                        className={styles.messageInput}
                                        value={entry.storageJson}
                                        onChange={event =>
                                          updateStateOverrideEntry(entry.id, current => ({
                                            ...current,
                                            storageJson: event.target.value,
                                          }))
                                        }
                                        placeholder="{}"
                                        spellCheck={false}
                                        disabled={isLoading}
                                        rows={8}
                                      />
                                    </label>
                                  )
                                ) : (
                                  <div className={styles.abiStatus}>Storage ABI not loaded</div>
                                )}
                              </>
                            ) : (
                              <label className={styles.field}>
                                <span className={styles.fieldLabel}>Data BOC</span>
                                <textarea
                                  className={styles.stateOverrideBoc}
                                  value={entry.dataBoc}
                                  onChange={event =>
                                    updateStateOverrideEntry(entry.id, current => ({
                                      ...current,
                                      dataBoc: event.target.value,
                                    }))
                                  }
                                  placeholder="Hex data cell BoC"
                                  spellCheck={false}
                                  disabled={isLoading}
                                  rows={5}
                                />
                              </label>
                            )}

                            {entry.storageSource === "abi" ? (
                              <div className={styles.previewPanel}>
                                <RawDataBlock
                                  variant="embedded"
                                  title={
                                    <span className={styles.previewTitle}>
                                      <span>Entered data as a cell</span>
                                      {storagePreview.error && (
                                        <span className={styles.previewError}>
                                          {storagePreview.error}
                                        </span>
                                      )}
                                    </span>
                                  }
                                  titleLabel="Entered data as a cell"
                                  collapsible
                                  defaultExpanded={false}
                                  value={storagePreview.dataBoc ?? ""}
                                  copyLabel="storage data BOC"
                                  empty={!storagePreview.dataBoc}
                                  emptyContent="No storage data built"
                                  maxHeight={140}
                                />
                              </div>
                            ) : (
                              storagePreview.error && (
                                <span className={styles.previewError}>{storagePreview.error}</span>
                              )
                            )}
                          </div>
                        )}
                      </div>
                    )}

                    {entry.stateKind === "frozen" && (
                      <label className={styles.field}>
                        <span className={styles.fieldLabel}>Frozen state hash</span>
                        <input
                          className={styles.textInput}
                          value={entry.frozenHash}
                          onChange={event =>
                            updateStateOverrideEntry(entry.id, current => ({
                              ...current,
                              frozenHash: event.target.value,
                            }))
                          }
                          placeholder="current state hash"
                          disabled={isLoading}
                        />
                      </label>
                    )}

                    <label className={styles.field}>
                      <span className={styles.fieldLabel}>Last transaction LT</span>
                      <input
                        className={styles.textInput}
                        value={entry.lastTransactionLt}
                        onChange={event =>
                          updateStateOverrideEntry(entry.id, current => ({
                            ...current,
                            lastTransactionLt: event.target.value,
                          }))
                        }
                        inputMode="numeric"
                        placeholder="0"
                        disabled={isLoading}
                      />
                    </label>

                    <label className={styles.field}>
                      <span className={styles.fieldLabel}>Last transaction hash</span>
                      <input
                        className={styles.textInput}
                        value={entry.lastTransactionHash}
                        onChange={event =>
                          updateStateOverrideEntry(entry.id, current => ({
                            ...current,
                            lastTransactionHash: event.target.value,
                          }))
                        }
                        placeholder="current transaction hash"
                        disabled={isLoading}
                      />
                    </label>
                  </div>
                )}
              </section>
            )
          })}

          <Button
            type="button"
            variant="outline"
            size="sm"
            leadingIcon={<Plus size={14} />}
            onClick={handleAddStateOverrideEntry}
            disabled={isLoading}
            className={styles.addOverrideButton}
          >
            Add account
          </Button>
        </div>
      )}
    </div>
  )

  const simulationForm = (
    <form className={styles.formPanel} onSubmit={event => void handleSubmit(event)}>
      <ContentTabs<EmulateInputMode>
        ariaLabel="Emulation input mode"
        tabs={[
          {
            label: (
              <span className={styles.inputModeLabel}>
                <WandSparkles size={15} aria-hidden="true" />
                Builder
              </span>
            ),
            value: "builder",
          },
          {
            label: (
              <span className={styles.inputModeLabel}>
                <FileJson size={15} aria-hidden="true" />
                Raw BOC
              </span>
            ),
            value: "raw",
          },
        ]}
        value={inputMode}
        onValueChange={setInputMode}
        panelClassName={styles.inputModePanel}
      >
        {inputMode === "builder" ? (
          <div className={styles.builderGrid}>
            <section className={styles.builderPanel}>
              <div className={styles.panelTitleRow}>
                <h2 className={styles.panelTitle}>Transaction</h2>
              </div>

              <Input
                fieldClassName={styles.field}
                className={styles.addressInput}
                label="Target contract"
                value={targetAddress}
                onChange={event => setTargetAddress(event.target.value)}
                placeholder="EQ..."
                disabled={isLoading}
              />

              <div className={styles.segmentedControl} aria-label="Incoming message type">
                <button
                  type="button"
                  className={`${styles.segment} ${
                    messageTransport === "internal" ? styles.segmentActive : ""
                  }`}
                  onClick={() => setMessageTransport("internal")}
                  aria-pressed={messageTransport === "internal"}
                  disabled={isLoading}
                >
                  Internal
                </button>
                <button
                  type="button"
                  className={`${styles.segment} ${
                    messageTransport === "external" ? styles.segmentActive : ""
                  }`}
                  onClick={() => setMessageTransport("external")}
                  aria-pressed={messageTransport === "external"}
                  disabled={isLoading}
                >
                  External
                </button>
              </div>

              {messageTransport === "internal" && (
                <div className={styles.messageOptions}>
                  <div className={styles.internalGrid}>
                    <Input
                      fieldClassName={styles.field}
                      className={styles.addressInput}
                      label="Source account"
                      value={sourceAddress}
                      onChange={event => setSourceAddress(event.target.value)}
                      placeholder="EQ..."
                      disabled={isLoading}
                    />
                    <Input
                      fieldClassName={styles.field}
                      label="Value"
                      suffix="GRAM"
                      value={messageValue}
                      onChange={event => setMessageValue(event.target.value)}
                      inputMode="decimal"
                      placeholder="0.05"
                      disabled={isLoading}
                    />
                  </div>
                  <Checkbox
                    checked={bounce}
                    onChange={event => setBounce(event.target.checked)}
                    disabled={isLoading}
                    label="Bounce"
                    className={styles.optionCheckbox}
                  />
                </div>
              )}

              <Input
                fieldClassName={styles.blockField}
                label="Masterchain block"
                value={mcSeqnoInput}
                onChange={event => setMcSeqnoInput(event.target.value)}
                inputMode="numeric"
                placeholder="latest"
                disabled={isLoading}
              />

              <Checkbox
                checked={ignoreChksig}
                onChange={event => setIgnoreChksig(event.target.checked)}
                disabled={isLoading}
                label="Ignore CHKSIG"
                description="Skip signature checks during emulation"
                className={styles.optionCheckbox}
              />
            </section>

            <section className={`${styles.builderPanel} ${styles.payloadPanel}`}>
              <div className={styles.panelTitleRow}>
                <h2 className={styles.panelTitle}>Payload</h2>
                <div className={styles.sourceSwitch} aria-label="ABI source">
                  <button
                    type="button"
                    className={`${styles.sourceSwitchButton} ${
                      abiSourceMode === "auto" ? styles.sourceSwitchButtonActive : ""
                    }`}
                    onClick={() => setAbiSourceMode("auto")}
                    aria-pressed={abiSourceMode === "auto"}
                  >
                    Auto
                  </button>
                  <button
                    type="button"
                    className={`${styles.sourceSwitchButton} ${
                      abiSourceMode === "manual" ? styles.sourceSwitchButtonActive : ""
                    }`}
                    onClick={() => setAbiSourceMode("manual")}
                    aria-pressed={abiSourceMode === "manual"}
                  >
                    JSON
                  </button>
                </div>
              </div>

              {abiSourceMode === "manual" ? (
                <label className={styles.field}>
                  <span className={styles.fieldLabel}>ABI JSON</span>
                  <textarea
                    className={styles.abiInput}
                    value={manualAbiJson}
                    onChange={event => setManualAbiJson(event.target.value)}
                    placeholder='{"contract_name":"..."}'
                    spellCheck={false}
                    disabled={isLoading}
                    rows={7}
                  />
                </label>
              ) : (
                <div className={styles.abiStatus}>
                  <span>
                    {abiLoadState.type === "ready"
                      ? abiLoadState.label
                      : abiLoadState.type === "error"
                        ? abiLoadState.message
                        : abiLoadState.type === "loading"
                          ? "Loading ABI"
                          : "ABI not loaded"}
                  </span>
                  {abiLoadState.type === "error" && (
                    <Button
                      type="button"
                      variant="secondary"
                      size="sm"
                      disabled={isLoading || !targetAddress.trim()}
                      onClick={() => void handleLoadAbi()}
                    >
                      Retry
                    </Button>
                  )}
                </div>
              )}

              {abiParseError && (
                <div className={styles.inlineError} role="alert">
                  {abiParseError}
                </div>
              )}

              <Select
                fieldClassName={styles.field}
                label="Message"
                value={selectedMessageId}
                onChange={event => handleMessageOptionChange(event.target.value)}
                disabled={isLoading || builderOptions.length === 0}
              >
                {builderOptions.length === 0 ? (
                  <option value="">No {messageTransport} ABI messages</option>
                ) : (
                  builderOptions.map(option => (
                    <option key={option.id} value={option.id}>
                      {formatAbiMessageOptionSummary(option)}
                    </option>
                  ))
                )}
              </Select>

              {selectedBuilderOption && (
                <>
                  <div className={styles.payloadInputHeader}>
                    <span className={styles.fieldLabel}>Arguments</span>
                    <div className={styles.sourceSwitch} aria-label="Arguments input mode">
                      <button
                        type="button"
                        className={`${styles.sourceSwitchButton} ${
                          argsInputMode === "form" ? styles.sourceSwitchButtonActive : ""
                        }`}
                        onClick={() => handleArgsInputModeChange("form")}
                        aria-pressed={argsInputMode === "form"}
                        disabled={isLoading || !messageSymbols}
                      >
                        Form
                      </button>
                      <button
                        type="button"
                        className={`${styles.sourceSwitchButton} ${
                          argsInputMode === "json" ? styles.sourceSwitchButtonActive : ""
                        }`}
                        onClick={() => handleArgsInputModeChange("json")}
                        aria-pressed={argsInputMode === "json"}
                        disabled={isLoading}
                      >
                        JSON
                      </button>
                    </div>
                  </div>

                  {argsInputMode === "form" && messageSymbols ? (
                    <AbiValueEditor
                      symbols={messageSymbols}
                      tyIdx={selectedBuilderOption.valueTyIdx}
                      value={argsFormValue}
                      onChange={handleArgsFormChange}
                      disabled={isLoading}
                    />
                  ) : (
                    <label className={styles.field}>
                      <span className={styles.fieldLabel}>Arguments JSON</span>
                      <textarea
                        className={styles.messageInput}
                        value={argsJson}
                        onChange={event => setArgsJson(event.target.value)}
                        placeholder="{}"
                        spellCheck={false}
                        disabled={isLoading}
                        rows={8}
                      />
                    </label>
                  )}

                  <div className={styles.previewPanel}>
                    <RawDataBlock
                      variant="embedded"
                      title={
                        <span className={styles.previewTitle}>
                          <span>Entered message as a cell</span>
                          {builderPreview.error && (
                            <span className={styles.previewError} title={builderPreview.error}>
                              {builderPreview.error}
                            </span>
                          )}
                        </span>
                      }
                      titleLabel="Entered message as a cell"
                      collapsible
                      defaultExpanded={false}
                      value={builderPreview.boc}
                      copyLabel="message BOC"
                      empty={!builderPreview.boc}
                      emptyContent="No message built"
                      maxHeight={160}
                    />
                  </div>
                </>
              )}
            </section>
            <section className={styles.builderOverrides} aria-label="State overrides">
              {stateOverrideControls}
            </section>
          </div>
        ) : (
          <div className={styles.rawModeGrid}>
            <label className={styles.field}>
              <span className={styles.fieldLabel}>Message BOC</span>
              <textarea
                className={styles.messageInput}
                value={rawMessage}
                onChange={event => setRawMessage(event.target.value)}
                placeholder="Hex message BoC"
                spellCheck={false}
                disabled={isLoading}
                rows={8}
              />
            </label>

            <Input
              fieldClassName={styles.blockField}
              label="Masterchain block"
              value={mcSeqnoInput}
              onChange={event => setMcSeqnoInput(event.target.value)}
              inputMode="numeric"
              placeholder="latest"
              disabled={isLoading}
            />

            <Checkbox
              checked={ignoreChksig}
              onChange={event => setIgnoreChksig(event.target.checked)}
              disabled={isLoading}
              label="Ignore CHKSIG"
              description="Skip signature checks during emulation"
              className={styles.optionCheckbox}
            />

            {stateOverrideControls}
          </div>
        )}

        <div className={styles.formFooter}>
          <div className={styles.actions}>
            <Button
              type="button"
              variant="ghost"
              leadingIcon={<RotateCcw size={16} />}
              onClick={handleReset}
              disabled={isLoading}
            >
              Reset
            </Button>
            <Button
              type="submit"
              variant="primary"
              leadingIcon={<Play size={16} />}
              loading={isLoading}
              disabled={!canEmulate}
            >
              Emulate
            </Button>
          </div>
        </div>
      </ContentTabs>
    </form>
  )

  const simulationHeader = (
    <header className={styles.header}>
      <div>
        <h1 className={styles.title}>Emulate Transaction</h1>
        {state.type === "ready" && state.mcSeqno !== undefined && (
          <div className={styles.metaLine}>
            <span>{`Block ${state.mcSeqno}`}</span>
          </div>
        )}
      </div>
    </header>
  )

  if (state.type === "ready") {
    return (
      <div className={`${styles.page} ${styles.pageReady}`}>
        <section className={styles.resultSection} aria-label="Emulation result">
          <TransactionTraceView
            hash={selectedTraceHash}
            traces={state.enrichment.transactions}
            contracts={contracts}
            compilerAbisByCodeHash={compilerAbisByCodeHash}
            verifiedSourcesByCodeHash={verifiedSourcesByCodeHash}
            valueFlow={valueFlow}
            activeTab={activeTab}
            traceOverview={traceOverviewDataFromTrace(state.result.trace)}
            statusLabels={{
              success: "Emulated transaction",
              error: "Emulation failed",
            }}
            breadcrumbs={[
              {label: "Emulate", path: routes.emulatePath},
              {label: selectedTraceHash || state.result.result.rootTxHash, isHash: true},
            ]}
            onTabChange={setActiveTab}
            onContractClick={handleContractClick}
            onBlockClick={handleBlockClick}
            onTransactionSelect={tx => setSelectedHash(transactionHashHex(tx))}
            loadActions={loadEmulatedActions}
            renderSelectedTransactionExtra={renderDebugPanel}
            renderSelectedTransactionMessageRouteAction={renderDebugAction}
          />
        </section>

        <section className={styles.secondaryControls} aria-label="Simulation input">
          {simulationHeader}
          {simulationForm}
        </section>
      </div>
    )
  }

  return (
    <div className={styles.page}>
      <ExplorerBreadcrumbs items={[{label: "Emulate"}]} />
      {simulationHeader}
      {simulationForm}
    </div>
  )
}

function parseMcSeqno(value: string): number | undefined {
  const trimmed = value.trim()
  if (!trimmed) {
    return undefined
  }
  if (!/^\d+$/.test(trimmed)) {
    throw new Error("Masterchain block must be a non-negative integer")
  }
  const seqno = Number(trimmed)
  if (!Number.isSafeInteger(seqno)) {
    throw new Error("Masterchain block is too large")
  }
  return seqno
}

function parseManualAbi(value: string): {readonly abi?: ContractABI; readonly error?: string} {
  const trimmed = value.trim()
  if (!trimmed) {
    return {}
  }

  try {
    const parsed = JSON.parse(trimmed) as Partial<ContractABI>
    if (!parsed || typeof parsed !== "object") {
      return {error: "ABI JSON must be an object"}
    }
    if (!parsed.contract_name || !Array.isArray(parsed.declarations)) {
      return {error: "ABI JSON is missing contract_name or declarations"}
    }
    return {abi: parsed as ContractABI}
  } catch (error) {
    return {error: error instanceof Error ? error.message : "Invalid ABI JSON"}
  }
}

function createAccountStateOverrideDraft({
  id,
  address,
}: {
  readonly id: string
  readonly address: string
}): AccountStateOverrideDraft {
  const storageJson = "{}"
  return {
    id,
    address,
    abi: undefined,
    loadState: {type: "idle"},
    mode: "fields",
    shardAccountBoc: "",
    balance: "",
    stateKind: "keep",
    codeBoc: "",
    storageEnabled: false,
    storageSource: "abi",
    storageInputMode: "form",
    storageJson,
    storageFormValue: parseAbiJson(storageJson, {}),
    dataBoc: "",
    frozenHash: "",
    lastTransactionLt: "",
    lastTransactionHash: "",
  }
}

function hydrateAccountStateOverrideDraft(
  entry: AccountStateOverrideDraft,
  address: string,
  shardAccount: ShardAccount,
  shardAccountCell: Cell,
  abi: ContractABI | undefined,
): AccountStateOverrideDraft {
  const account = shardAccount.account
  const accountState = account?.storage.state
  const codeBoc =
    accountState?.type === "active" && accountState.state.code
      ? accountState.state.code.toBoc().toString("hex")
      : ""
  const dataBoc =
    accountState?.type === "active" && accountState.state.data
      ? accountState.state.data.toBoc().toString("hex")
      : ""
  const storage = currentStorageOverrideDraft(entry, abi, dataBoc)

  return {
    ...entry,
    address,
    abi,
    loadedAddress: address,
    loadState: {type: "ready"},
    mode: "fields",
    shardAccountBoc: shardAccountCell.toBoc().toString("hex"),
    balance: fromNano(account?.storage.balance.coins ?? 0n),
    stateKind: accountState?.type ?? "uninit",
    codeBoc,
    storageEnabled: storage.storageEnabled,
    storageSource: storage.storageSource,
    storageInputMode: storage.storageInputMode,
    storageJson: storage.storageJson,
    storageFormValue: storage.storageFormValue,
    dataBoc,
    frozenHash: accountState?.type === "frozen" ? accountState.stateHash.toString() : "",
    lastTransactionLt: shardAccount.lastTransactionLt.toString(),
    lastTransactionHash: shardAccount.lastTransactionHash.toString(),
  }
}

function currentStorageOverrideDraft(
  entry: AccountStateOverrideDraft,
  abi: ContractABI | undefined,
  dataBoc: string,
): Pick<
  AccountStateOverrideDraft,
  "storageEnabled" | "storageSource" | "storageInputMode" | "storageJson" | "storageFormValue"
> {
  if (!dataBoc) {
    return {
      storageEnabled: false,
      storageSource: entry.storageSource,
      storageInputMode: entry.storageInputMode,
      storageJson: entry.storageJson,
      storageFormValue: entry.storageFormValue,
    }
  }

  if (abi) {
    try {
      const storageJson = stringifyAbiJson(decodeAbiStorageDataBoc(abi, dataBoc))
      return {
        storageEnabled: true,
        storageSource: "abi",
        storageInputMode: "form",
        storageJson,
        storageFormValue: parseAbiJson(storageJson, {}),
      }
    } catch {
      // Fall through to raw BOC when the loaded contract state does not match the active ABI.
    }
  }

  return {
    storageEnabled: true,
    storageSource: "raw",
    storageInputMode: entry.storageInputMode,
    storageJson: entry.storageJson,
    storageFormValue: entry.storageFormValue,
  }
}

function buildAccountStateOverrides({
  enabled,
  entries,
  storagePreviews,
}: {
  readonly enabled: boolean
  readonly entries: readonly AccountStateOverrideDraft[]
  readonly storagePreviews: ReadonlyMap<
    string,
    {readonly dataBoc?: string; readonly error?: string}
  >
}): RawMessageEmulationOptions["accountStateOverrides"] | undefined {
  if (!enabled) {
    return undefined
  }

  if (entries.length === 0) {
    throw new Error("At least one override account is required")
  }

  const overrides: NonNullable<RawMessageEmulationOptions["accountStateOverrides"]> = {}
  const seenAddresses = new Set<string>()

  for (const entry of entries) {
    const normalizedAddress = entry.address.trim()
    if (!normalizedAddress) {
      throw new Error("Override account address is required")
    }

    if (!isValidAddress(normalizedAddress)) {
      throw new Error(`Invalid override account address: ${normalizedAddress}`)
    }

    const normalizedAddressKey = addressKey(normalizedAddress)
    if (seenAddresses.has(normalizedAddressKey)) {
      throw new Error(`Duplicate override account: ${normalizedAddress}`)
    }
    seenAddresses.add(normalizedAddressKey)

    if (entry.mode === "shardAccount") {
      const normalizedShardAccountBoc = entry.shardAccountBoc.trim()
      if (!normalizedShardAccountBoc) {
        throw new Error("ShardAccount BOC is required")
      }

      overrides[normalizedAddress] = {
        shardAccountBoc: normalizedShardAccountBoc,
      }
      continue
    }

    const preview = storagePreviews.get(entry.id)
    if (preview?.error) {
      throw new Error(preview.error)
    }

    const override: NonNullable<RawMessageEmulationOptions["accountStateOverrides"]>[string] = {}
    const normalizedBalance = entry.balance.trim()
    const normalizedLastTransactionLt = entry.lastTransactionLt.trim()
    const normalizedLastTransactionHash = entry.lastTransactionHash.trim()
    if (normalizedBalance) {
      try {
        override.balance = toNano(normalizedBalance).toString()
      } catch {
        throw new Error("Override balance must be a valid GRAM amount")
      }
    }
    if (normalizedLastTransactionLt) {
      override.lastTransactionLt = normalizedLastTransactionLt
    }
    if (normalizedLastTransactionHash) {
      override.lastTransactionHash = normalizedLastTransactionHash
    }

    const normalizedCodeBoc = entry.codeBoc.trim()
    const normalizedDataBoc = entry.storageEnabled ? preview?.dataBoc?.trim() : undefined
    if (entry.stateKind === "uninit") {
      override.state = {type: "uninit"}
    } else if (entry.stateKind === "frozen") {
      override.state = {
        type: "frozen",
        stateHash: entry.frozenHash.trim() || undefined,
      }
    } else if (entry.stateKind === "active" || normalizedCodeBoc || normalizedDataBoc) {
      override.state = {
        type: "active",
        codeBoc: normalizedCodeBoc || undefined,
        dataBoc: normalizedDataBoc || undefined,
      }
    }

    overrides[normalizedAddress] = override
  }

  return overrides
}

function buildStorageOverridePreview({
  enabled,
  source,
  abi,
  storageJson,
  rawDataBoc,
}: {
  readonly enabled: boolean
  readonly source: StorageOverrideSource
  readonly abi: ContractABI | undefined
  readonly storageJson: string
  readonly rawDataBoc: string
}): {readonly dataBoc?: string; readonly error?: string} {
  if (!enabled) {
    return {}
  }

  try {
    if (source === "raw") {
      const trimmed = rawDataBoc.trim()
      if (!trimmed) {
        return {error: "Data BOC is required"}
      }
      return {dataBoc: parseAbiCellArg(trimmed).toBoc().toString("hex")}
    }

    if (!abi) {
      return {error: "Storage ABI is not loaded"}
    }

    return {dataBoc: buildAbiStorageDataBoc(abi, storageJson)}
  } catch (error) {
    return {
      error: error instanceof Error ? error.message : "Failed to build storage data",
    }
  }
}

function isValidAddress(value: string): boolean {
  if (!value.trim()) {
    return false
  }
  try {
    Address.parse(value.trim())
    return true
  } catch {
    return false
  }
}

function buildBuilderPreview({
  abi,
  option,
  destination,
  source,
  value,
  bounce,
  argsJson,
}: {
  readonly abi: ContractABI | undefined
  readonly option: AbiMessageBuilderOption | undefined
  readonly destination: string
  readonly source: string
  readonly value: string
  readonly bounce: boolean
  readonly argsJson: string
}): {readonly boc: string; readonly error?: string} {
  if (!abi || !option || !destination.trim()) {
    return {boc: ""}
  }

  try {
    return {
      boc: buildAbiMessageBoc({
        abi,
        option,
        destination,
        source,
        value,
        bounce,
        argsJson,
      }),
    }
  } catch (error) {
    return {
      boc: "",
      error: error instanceof Error ? error.message : "Failed to build message",
    }
  }
}

import {type FormEvent, useCallback, useEffect, useMemo, useRef, useState} from "react"
import {
  Bug,
  Clock3,
  Database,
  FileJson,
  Minus,
  Play,
  Plus,
  RefreshCw,
  RotateCcw,
  SlidersHorizontal,
  Trash2,
  WandSparkles,
} from "lucide-react"
import {
  Button,
  Checkbox,
  ContentTabs,
  InlineAction,
  InlineButton,
  Input,
  Select,
  useToast,
} from "@acton/ui"
import {
  AbiValueEditor,
  TonAddressInput,
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
  type TonAddressSuggestion,
} from "@acton/transaction-ui"
import {useNavigate, useSearchParams} from "react-router-dom"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {Address, Cell, fromNano, loadShardAccount, toNano, type ShardAccount} from "@ton/core"

import {useNetworkInfo} from "../hooks/useNetworkInfo"
import {useAddressFormat} from "../hooks/useNetworkInfo"
import {useAddressBook} from "../hooks/useAddressBook"
import {useFavoriteAccounts} from "../hooks/useFavoriteAccounts"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {openExplorerPath, type ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"
import {formatAddress, normalizeAddress} from "../components/utils"
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
type AccountStateOverrideKind = "keep" | "active" | "uninit" | "frozen"
type StorageOverrideSource = "abi" | "raw"
type TimeOverrideMode = "increase" | "timestamp"
const EMULATE_ADDRESS_QUERY_PARAM = "address"
const EMULATE_SOURCE_QUERY_PARAM = "source"
const EMULATE_VALUE_QUERY_PARAM = "value"
const EMULATE_BOUNCE_QUERY_PARAM = "bounce"
const EMULATE_MC_SEQNO_QUERY_PARAM = "mcSeqno"
const EMULATE_IGNORE_CHKSIG_QUERY_PARAM = "ignoreChksig"
const EMULATE_TIME_MODE_QUERY_PARAM = "timeMode"
const EMULATE_INCREASE_TIME_QUERY_PARAM = "increaseTime"
const EMULATE_TIMESTAMP_QUERY_PARAM = "timestamp"
const DEFAULT_MESSAGE_VALUE = "0.5"
const MAX_UINT32 = 0xff_ff_ff_ff

interface EmulateSearchFields {
  readonly targetAddress: string
  readonly sourceAddress: string
  readonly messageValue: string
  readonly bounce: boolean
  readonly mcSeqnoInput: string
  readonly ignoreChksig: boolean
  readonly timeOverrideMode: TimeOverrideMode
  readonly increaseTimeInput: string
  readonly unixTimestampInput: string
}

function readEmulateSearchFields(searchParams: URLSearchParams): EmulateSearchFields {
  const timeMode = searchParams.get(EMULATE_TIME_MODE_QUERY_PARAM)
  return {
    targetAddress: searchParams.get(EMULATE_ADDRESS_QUERY_PARAM) ?? "",
    sourceAddress: searchParams.get(EMULATE_SOURCE_QUERY_PARAM) ?? "",
    messageValue: searchParams.get(EMULATE_VALUE_QUERY_PARAM) ?? DEFAULT_MESSAGE_VALUE,
    bounce: searchParams.get(EMULATE_BOUNCE_QUERY_PARAM) !== "false",
    mcSeqnoInput: searchParams.get(EMULATE_MC_SEQNO_QUERY_PARAM) ?? "",
    ignoreChksig: searchParams.get(EMULATE_IGNORE_CHKSIG_QUERY_PARAM) === "true",
    timeOverrideMode: timeMode === "increase" ? "increase" : "timestamp",
    increaseTimeInput: searchParams.get(EMULATE_INCREASE_TIME_QUERY_PARAM) ?? "",
    unixTimestampInput: searchParams.get(EMULATE_TIMESTAMP_QUERY_PARAM) ?? "",
  }
}

function areEmulateSearchFieldsEqual(
  left: EmulateSearchFields,
  right: EmulateSearchFields,
): boolean {
  return (
    left.targetAddress === right.targetAddress &&
    left.sourceAddress === right.sourceAddress &&
    left.messageValue === right.messageValue &&
    left.bounce === right.bounce &&
    left.mcSeqnoInput === right.mcSeqnoInput &&
    left.ignoreChksig === right.ignoreChksig &&
    left.timeOverrideMode === right.timeOverrideMode &&
    left.increaseTimeInput === right.increaseTimeInput &&
    left.unixTimestampInput === right.unixTimestampInput
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
  readonly balance: string
  readonly stateKind: AccountStateOverrideKind
  readonly currentStateKind?: Exclude<AccountStateOverrideKind, "keep">
  readonly codeBoc: string
  readonly storageEnabled: boolean
  readonly storageSource: StorageOverrideSource
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
  const {fetchName, getCachedName, prefetchNames} = useAddressBook()
  const {favorites} = useFavoriteAccounts()
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
  const [bounce, setBounce] = useState(() => readEmulateSearchFields(searchParams).bounce)
  const [abiSourceMode, setAbiSourceMode] = useState<AbiSourceMode>("auto")
  const [manualAbiJson, setManualAbiJson] = useState("")
  const [loadedAbi, setLoadedAbi] = useState<ContractABI | undefined>()
  const [abiLoadState, setAbiLoadState] = useState<AbiLoadState>({type: "idle"})
  const [selectedMessageId, setSelectedMessageId] = useState("")
  const [argsJson, setArgsJson] = useState("{}")
  const [argsFormValue, setArgsFormValue] = useState<unknown>({})
  const [rawMessage, setRawMessage] = useState("")
  const [mcSeqnoInput, setMcSeqnoInput] = useState(
    () => readEmulateSearchFields(searchParams).mcSeqnoInput,
  )
  const [ignoreChksig, setIgnoreChksig] = useState(
    () => readEmulateSearchFields(searchParams).ignoreChksig,
  )
  const [timeOverrideOpen, setTimeOverrideOpen] = useState(false)
  const [timeOverrideMode, setTimeOverrideMode] = useState<TimeOverrideMode>(
    () => readEmulateSearchFields(searchParams).timeOverrideMode,
  )
  const [increaseTimeInput, setIncreaseTimeInput] = useState(
    () => readEmulateSearchFields(searchParams).increaseTimeInput,
  )
  const [unixTimestampInput, setUnixTimestampInput] = useState(
    () => readEmulateSearchFields(searchParams).unixTimestampInput,
  )
  const [baseBlockUnixTime, setBaseBlockUnixTime] = useState<number | undefined>()
  const [stateOverrideEntries, setStateOverrideEntries] = useState<
    readonly AccountStateOverrideDraft[]
  >([])
  const stateOverrideEnabled = stateOverrideEntries.length > 0
  const [state, setState] = useState<EmulateState>({type: "idle"})
  const [activeTab, setActiveTab] = useState<TransactionTraceTabType>("value-flow")
  const [selectedHash, setSelectedHash] = useState<string | undefined>()
  const [expandedDebugHash, setExpandedDebugHash] = useState<string | undefined>()
  const latestAbiLoadRequest = useRef(0)
  const nextStateOverrideId = useRef(1)
  const baseBlockUnixTimeQuery = useRef<string | undefined>(undefined)
  const lastSearchFields = useRef(readEmulateSearchFields(searchParams))
  const isApplyingSearchFields = useRef(false)

  useEffect(() => {
    void prefetchNames(favorites.map(favorite => favorite.address))
  }, [favorites, prefetchNames])

  useEffect(() => {
    if (!timeOverrideOpen) {
      return
    }

    let mcSeqno: number | undefined
    try {
      mcSeqno = parseMcSeqno(mcSeqnoInput)
    } catch {
      baseBlockUnixTimeQuery.current = undefined
      setBaseBlockUnixTime(undefined)
      return
    }

    const queryKey = `${network}:${mcSeqno === undefined ? "latest" : mcSeqno}`
    if (baseBlockUnixTimeQuery.current === queryKey && baseBlockUnixTime !== undefined) {
      return
    }

    let cancelled = false
    void loadEmulationBlockUnixTime(client, mcSeqno)
      .then(unixTime => {
        if (!cancelled) {
          baseBlockUnixTimeQuery.current = queryKey
          setBaseBlockUnixTime(unixTime)
        }
      })
      .catch(() => {
        if (!cancelled) {
          baseBlockUnixTimeQuery.current = undefined
          setBaseBlockUnixTime(undefined)
        }
      })

    return () => {
      cancelled = true
    }
  }, [baseBlockUnixTime, client, mcSeqnoInput, network, timeOverrideOpen])

  const favoriteAddressSuggestions: readonly TonAddressSuggestion[] = favorites.map(favorite => {
    const name = getCachedName(favorite.address)
    const fullAddress = formatAddress(favorite.address, false, addressFormat)
    const displayAddress = name ? formatAddress(favorite.address, true, addressFormat) : fullAddress
    return {
      address: fullAddress,
      label: name ? `${name} · ${displayAddress}` : displayAddress,
    }
  })

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
  const hasValidTargetAddress = isValidAddress(targetAddress.trim())
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
        stateOverrideEntries.map(entry => {
          const context = stateOverrideAbiContexts.get(entry.id)
          const source: StorageOverrideSource =
            entry.storageSource === "abi" && context?.storageInfo && context.storageSymbols
              ? "abi"
              : "raw"
          return [
            entry.id,
            buildStorageOverridePreview({
              enabled: stateOverrideEnabled && entry.storageEnabled,
              source,
              abi: context?.abi,
              storageJson: entry.storageJson,
              rawDataBoc: entry.dataBoc,
            }),
          ]
        }),
      ),
    [stateOverrideAbiContexts, stateOverrideEnabled, stateOverrideEntries],
  )
  const stateOverrideAccountCount = stateOverrideEntries.length
  const timeOverrideInput = timeOverrideMode === "increase" ? increaseTimeInput : unixTimestampInput
  const parsedTimeOverrideInput = parseOptionalUint32(timeOverrideInput)
  const previewUnixTime =
    parsedTimeOverrideInput === undefined
      ? undefined
      : timeOverrideMode === "timestamp"
        ? parsedTimeOverrideInput
        : baseBlockUnixTime === undefined
          ? undefined
          : baseBlockUnixTime + parsedTimeOverrideInput
  const timeOverrideInvalid =
    timeOverrideInput.trim().length > 0 &&
    (parsedTimeOverrideInput === undefined ||
      (timeOverrideMode === "increase" &&
        previewUnixTime !== undefined &&
        previewUnixTime > MAX_UINT32))
  const canApplyStateOverride =
    !stateOverrideEnabled ||
    (stateOverrideEntries.length > 0 &&
      stateOverrideEntries.every(entry => {
        const preview = stateOverrideStoragePreviews.get(entry.id)
        return (
          Boolean(entry.address.trim()) &&
          entry.loadState.type !== "loading" &&
          preview?.error === undefined
        )
      }))
  const canEmulate =
    canApplyStateOverride &&
    !timeOverrideInvalid &&
    (inputMode === "builder"
      ? Boolean(builderPreview.boc) && builderPreview.error === undefined
      : Boolean(rawMessage.trim()))

  useEffect(() => {
    const fieldsFromUrl = readEmulateSearchFields(searchParams)
    const previousFields = lastSearchFields.current
    if (areEmulateSearchFieldsEqual(fieldsFromUrl, previousFields)) {
      return
    }

    lastSearchFields.current = fieldsFromUrl
    isApplyingSearchFields.current = true
    setTargetAddress(fieldsFromUrl.targetAddress)
    setSourceAddress(fieldsFromUrl.sourceAddress)
    setMessageValue(fieldsFromUrl.messageValue)
    setBounce(fieldsFromUrl.bounce)
    if (fieldsFromUrl.mcSeqnoInput !== previousFields.mcSeqnoInput) {
      baseBlockUnixTimeQuery.current = undefined
      setBaseBlockUnixTime(undefined)
    }
    setMcSeqnoInput(fieldsFromUrl.mcSeqnoInput)
    setIgnoreChksig(fieldsFromUrl.ignoreChksig)
    setTimeOverrideMode(fieldsFromUrl.timeOverrideMode)
    setIncreaseTimeInput(fieldsFromUrl.increaseTimeInput)
    setUnixTimestampInput(fieldsFromUrl.unixTimestampInput)
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
      bounce,
      mcSeqnoInput: mcSeqnoInput.trim(),
      ignoreChksig,
      timeOverrideMode,
      increaseTimeInput: increaseTimeInput.trim(),
      unixTimestampInput: unixTimestampInput.trim(),
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
    if (nextFields.bounce) {
      nextParams.delete(EMULATE_BOUNCE_QUERY_PARAM)
    } else {
      nextParams.set(EMULATE_BOUNCE_QUERY_PARAM, "false")
    }
    if (nextFields.mcSeqnoInput) {
      nextParams.set(EMULATE_MC_SEQNO_QUERY_PARAM, nextFields.mcSeqnoInput)
    } else {
      nextParams.delete(EMULATE_MC_SEQNO_QUERY_PARAM)
    }
    if (nextFields.ignoreChksig) {
      nextParams.set(EMULATE_IGNORE_CHKSIG_QUERY_PARAM, "true")
    } else {
      nextParams.delete(EMULATE_IGNORE_CHKSIG_QUERY_PARAM)
    }
    if (nextFields.timeOverrideMode === "increase") {
      nextParams.set(EMULATE_TIME_MODE_QUERY_PARAM, "increase")
    } else {
      nextParams.delete(EMULATE_TIME_MODE_QUERY_PARAM)
    }
    if (nextFields.increaseTimeInput) {
      nextParams.set(EMULATE_INCREASE_TIME_QUERY_PARAM, nextFields.increaseTimeInput)
    } else {
      nextParams.delete(EMULATE_INCREASE_TIME_QUERY_PARAM)
    }
    if (nextFields.unixTimestampInput) {
      nextParams.set(EMULATE_TIMESTAMP_QUERY_PARAM, nextFields.unixTimestampInput)
    } else {
      nextParams.delete(EMULATE_TIMESTAMP_QUERY_PARAM)
    }
    lastSearchFields.current = nextFields
    setSearchParams(nextParams, {replace: true})
  }, [
    bounce,
    ignoreChksig,
    increaseTimeInput,
    mcSeqnoInput,
    messageValue,
    searchParams,
    setSearchParams,
    sourceAddress,
    targetAddress,
    timeOverrideMode,
    unixTimestampInput,
  ])

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
              ? hydrateAccountStateOverrideDraft(entry, address, shardAccount, accountAbi)
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
      setAbiSourceMode("auto")
      setLoadedAbi(undefined)
      setAbiLoadState({type: "idle"})
      return
    }

    try {
      Address.parse(address)
    } catch {
      setAbiSourceMode("auto")
      setLoadedAbi(undefined)
      setAbiLoadState({type: "idle"})
      return
    }

    setAbiSourceMode("auto")
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

  const handleArgsFormChange = useCallback((value: unknown) => {
    setArgsFormValue(value)
    setArgsJson(stringifyAbiJson(value))
  }, [])

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
      const now = await resolveEmulationUnixTime({
        client,
        mcSeqno,
        mode: timeOverrideMode,
        value: timeOverrideInput,
      })
      const result = await emulateRawMessageBoc(activeRawMessage, network, {
        accountStateOverrides,
        ignoreChksig,
        mcSeqno,
        now,
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
      globalThis.requestAnimationFrame(() => {
        globalThis.scrollTo({top: 0, behavior: "smooth"})
      })
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
    setArgsJson("{}")
    setArgsFormValue({})
    setRawMessage("")
    setMcSeqnoInput("")
    setIgnoreChksig(false)
    setTimeOverrideOpen(false)
    setTimeOverrideMode("timestamp")
    setIncreaseTimeInput("")
    setUnixTimestampInput("")
    setBaseBlockUnixTime(undefined)
    setStateOverrideEntries([])
    setSelectedHash(undefined)
    setExpandedDebugHash(undefined)
    setActiveTab("value-flow")
    setState({type: "idle"})
  }

  const stateOverrideControls = (
    <div className={styles.stateOverride}>
      {stateOverrideEnabled && (
        <div className={styles.stateOverrideBody}>
          {stateOverrideEntries.map((entry, index) => {
            const storagePreview = stateOverrideStoragePreviews.get(entry.id) ?? {}
            const storageContext = stateOverrideAbiContexts.get(entry.id)
            const storageInfo = storageContext?.storageInfo
            const storageSymbols = storageContext?.storageSymbols
            const usesStorageAbi =
              entry.storageSource === "abi" &&
              storageInfo !== undefined &&
              storageSymbols !== undefined
            const canLoadCurrent = isValidAddress(entry.address.trim())

            return (
              <section className={styles.stateOverrideAccount} key={entry.id}>
                <div className={styles.accountOverrideHeader}>
                  <div className={styles.accountOverrideTitle}>Account {index + 1}</div>
                  <InlineAction
                    label={`Remove account ${index + 1}`}
                    icon={<Trash2 />}
                    variant="danger"
                    onClick={() => handleRemoveStateOverrideEntry(entry.id)}
                    disabled={isLoading}
                  />
                </div>

                <TonAddressInput
                  className={styles.addressInput}
                  label="Address"
                  labelAction={
                    <InlineAction
                      className={styles.accountReloadAction}
                      label="Load current account state"
                      icon={<RefreshCw />}
                      onClick={() => void preloadStateOverrideEntry(entry.id, entry.address)}
                      disabled={isLoading || !canLoadCurrent || entry.loadState.type === "loading"}
                      aria-busy={entry.loadState.type === "loading"}
                    />
                  }
                  value={entry.address}
                  onValueChange={nextAddress =>
                    updateStateOverrideEntry(entry.id, current => ({
                      ...current,
                      address: nextAddress,
                      abi: current.loadedAddress === nextAddress.trim() ? current.abi : undefined,
                      loadedAddress:
                        current.loadedAddress === nextAddress.trim()
                          ? current.loadedAddress
                          : undefined,
                      currentStateKind:
                        current.loadedAddress === nextAddress.trim()
                          ? current.currentStateKind
                          : undefined,
                      loadState: {type: "idle"},
                    }))
                  }
                  placeholder={targetAddress.trim() ? "Target contract" : "EQ… or 0:…"}
                  suggestions={favoriteAddressSuggestions}
                  disabled={isLoading}
                />

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
                    <option value="keep">
                      Keep current
                      {entry.currentStateKind ? ` (${entry.currentStateKind})` : ""}
                    </option>
                    <option value="active">Active</option>
                    <option value="uninit">Uninit</option>
                    <option value="frozen">Frozen</option>
                  </Select>

                  {(entry.stateKind === "keep" || entry.stateKind === "active") && (
                    <div className={styles.stateOverrideNested}>
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
                          {usesStorageAbi ? (
                            <AbiValueEditor
                              symbols={storageSymbols}
                              tyIdx={storageInfo.tyIdx}
                              value={entry.storageFormValue}
                              onChange={value => handleStorageFormChange(entry.id, value)}
                              addressSuggestions={favoriteAddressSuggestions}
                              disabled={isLoading}
                            />
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

                          {!usesStorageAbi && storagePreview.error && (
                            <span className={styles.previewError}>{storagePreview.error}</span>
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
                </div>
              </section>
            )
          })}
        </div>
      )}

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
  )

  const stateOverrideOptions = (
    <details className={`${styles.advancedOptions} ${styles.stateOverrideOptions}`}>
      <summary className={styles.advancedOptionsSummary} aria-label="State overrides">
        <span className={styles.advancedOptionsTitle}>
          <Database size={17} aria-hidden="true" />
          State overrides
          {stateOverrideAccountCount > 0 && (
            <span className={styles.panelMeta}>
              {stateOverrideAccountCount === 1
                ? "1 account"
                : `${stateOverrideAccountCount} accounts`}
            </span>
          )}
        </span>
        <span className={styles.advancedOptionsToggle} aria-hidden="true">
          <Plus className={styles.advancedOptionsPlus} size={18} />
          <Minus className={styles.advancedOptionsMinus} size={18} />
        </span>
      </summary>
      <div className={styles.advancedOptionsContent}>{stateOverrideControls}</div>
    </details>
  )

  const timestampOverrideOptions = (
    <details
      className={styles.timeOverride}
      open={timeOverrideOpen}
      onToggle={event => setTimeOverrideOpen(event.currentTarget.open)}
    >
      <summary
        className={styles.timeOverrideSummary}
        aria-label="Override timestamp"
        onClick={() => {
          if (!timeOverrideOpen && timeOverrideMode === "timestamp" && !unixTimestampInput.trim()) {
            setUnixTimestampInput(String(baseBlockUnixTime ?? Math.floor(Date.now() / 1000)))
          }
        }}
      >
        <span className={styles.advancedOptionsTitle}>
          <Clock3 size={17} aria-hidden="true" />
          Override timestamp
        </span>
        <span className={styles.advancedOptionsToggle} aria-hidden="true">
          <Plus className={styles.timeOverridePlus} size={18} />
          <Minus className={styles.timeOverrideMinus} size={18} />
        </span>
      </summary>
      <div className={styles.timeOverrideContent}>
        <div className={styles.timeOverrideModes} role="radiogroup" aria-label="Timestamp mode">
          <label className={styles.timeOverrideMode}>
            <input
              className={styles.timeOverrideRadio}
              type="radio"
              name="time-override-mode"
              value="increase"
              checked={timeOverrideMode === "increase"}
              onChange={() => setTimeOverrideMode("increase")}
              disabled={isLoading}
            />
            Increase time
          </label>
          <label className={styles.timeOverrideMode}>
            <input
              className={styles.timeOverrideRadio}
              type="radio"
              name="time-override-mode"
              value="timestamp"
              checked={timeOverrideMode === "timestamp"}
              onChange={() => {
                setTimeOverrideMode("timestamp")
                setUnixTimestampInput(String(baseBlockUnixTime ?? Math.floor(Date.now() / 1000)))
              }}
              disabled={isLoading}
            />
            Set UNIX timestamp
          </label>
        </div>

        <Input
          aria-label={timeOverrideMode === "increase" ? "Seconds to add" : "UNIX timestamp"}
          value={timeOverrideInput}
          onChange={event => {
            if (timeOverrideMode === "increase") {
              setIncreaseTimeInput(event.target.value)
            } else {
              setUnixTimestampInput(event.target.value)
            }
          }}
          inputMode="numeric"
          placeholder={timeOverrideMode === "increase" ? "Seconds" : "UNIX timestamp"}
          invalid={timeOverrideInvalid}
          disabled={isLoading}
        />

        {baseBlockUnixTime !== undefined &&
          previewUnixTime !== undefined &&
          previewUnixTime <= MAX_UINT32 && (
            <div className={styles.timeOverridePreview}>
              <span>{formatEmulationUnixTime(baseBlockUnixTime)}</span>
              <span aria-hidden="true">→</span>
              <span>{formatEmulationUnixTime(previewUnixTime)}</span>
            </div>
          )}
      </div>
    </details>
  )

  const advancedEmulationOptions = (
    <details className={styles.advancedOptions}>
      <summary className={styles.advancedOptionsSummary} aria-label="Advanced options">
        <span className={styles.advancedOptionsTitle}>
          <SlidersHorizontal size={17} aria-hidden="true" />
          Advanced options
        </span>
        <span className={styles.advancedOptionsToggle} aria-hidden="true">
          <Plus className={styles.advancedOptionsPlus} size={18} />
          <Minus className={styles.advancedOptionsMinus} size={18} />
        </span>
      </summary>
      <div className={styles.advancedOptionsContent}>
        {messageTransport === "internal" && (
          <Checkbox
            checked={bounce}
            onChange={event => setBounce(event.target.checked)}
            disabled={isLoading}
            label="Bounce"
            description="Bounce failed messages back to From"
            className={styles.optionCheckbox}
          />
        )}

        <Checkbox
          checked={ignoreChksig}
          onChange={event => setIgnoreChksig(event.target.checked)}
          disabled={isLoading}
          label="Ignore CHKSIG"
          description="Skip signature checks during emulation"
          className={styles.optionCheckbox}
        />

        <Input
          fieldClassName={styles.blockField}
          label="Masterchain block"
          value={mcSeqnoInput}
          onChange={event => {
            baseBlockUnixTimeQuery.current = undefined
            setBaseBlockUnixTime(undefined)
            setMcSeqnoInput(event.target.value)
          }}
          inputMode="numeric"
          placeholder="latest"
          disabled={isLoading}
        />
      </div>
    </details>
  )

  const emulationOptions = (
    <div className={styles.emulationOptions}>
      {advancedEmulationOptions}
      {timestampOverrideOptions}
      {stateOverrideOptions}
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
                Raw
              </span>
            ),
            value: "raw",
          },
        ]}
        value={inputMode}
        onValueChange={setInputMode}
        panelClassName={styles.inputModePanel}
      >
        <Button
          className={styles.emulateAction}
          type="submit"
          variant="primary"
          size="sm"
          leadingIcon={<Play size={16} />}
          loading={isLoading}
          disabled={!canEmulate}
        >
          Emulate
        </Button>

        {inputMode === "builder" ? (
          <div className={styles.builderGrid}>
            <section className={styles.builderPanel}>
              <div className={`${styles.panelTitleRow} ${styles.transactionTitleRow}`}>
                <h2 className={styles.panelTitle}>Transaction</h2>
                <InlineAction
                  icon={<RotateCcw size={14} />}
                  label="Reset transaction fields"
                  title="Reset all transaction fields to their default values"
                  onClick={handleReset}
                  disabled={isLoading}
                />
              </div>

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
                <TonAddressInput
                  fieldClassName={styles.field}
                  className={styles.addressInput}
                  label="From"
                  value={sourceAddress}
                  onValueChange={setSourceAddress}
                  suggestions={favoriteAddressSuggestions}
                  disabled={isLoading}
                />
              )}

              <TonAddressInput
                fieldClassName={styles.field}
                className={styles.addressInput}
                label="To"
                value={targetAddress}
                onValueChange={setTargetAddress}
                suggestions={favoriteAddressSuggestions}
                disabled={isLoading}
              />

              {messageTransport === "internal" && (
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
              )}

              {emulationOptions}
            </section>

            <section className={`${styles.builderPanel} ${styles.payloadPanel}`}>
              <div className={styles.panelTitleRow}>
                <h2 className={styles.panelTitle}>Message</h2>
              </div>

              {!hasValidTargetAddress && (
                <div className={styles.messagePlaceholder}>
                  <span>Enter a valid contract address in To to configure the message</span>
                </div>
              )}

              {hasValidTargetAddress && abiSourceMode === "manual" && (
                <div className={styles.field}>
                  <div className={styles.payloadInputHeader}>
                    <label className={styles.fieldLabel} htmlFor="emulate-manual-abi-json">
                      ABI JSON
                    </label>
                    <InlineButton type="button" onClick={() => setAbiSourceMode("auto")}>
                      Cancel
                    </InlineButton>
                  </div>
                  <textarea
                    id="emulate-manual-abi-json"
                    className={styles.abiInput}
                    value={manualAbiJson}
                    onChange={event => setManualAbiJson(event.target.value)}
                    placeholder='{"contract_name":"..."}'
                    spellCheck={false}
                    disabled={isLoading}
                    rows={7}
                  />
                </div>
              )}

              {hasValidTargetAddress &&
                abiSourceMode === "auto" &&
                abiLoadState.type !== "ready" && (
                  <div className={styles.abiStatus}>
                    <span>
                      {abiLoadState.type === "error"
                        ? abiLoadState.message
                        : abiLoadState.type === "loading"
                          ? "Loading ABI"
                          : "ABI not loaded"}
                    </span>
                    {abiLoadState.type !== "loading" && (
                      <Button
                        type="button"
                        variant="secondary"
                        size="sm"
                        disabled={isLoading}
                        onClick={() => setAbiSourceMode("manual")}
                      >
                        Enter ABI manually
                      </Button>
                    )}
                  </div>
                )}

              {hasValidTargetAddress && abiParseError && (
                <div className={styles.inlineError} role="alert">
                  {abiParseError}
                </div>
              )}

              {hasValidTargetAddress && activeAbi && (
                <Select
                  fieldClassName={styles.field}
                  aria-label="Message"
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
              )}

              {hasValidTargetAddress && selectedBuilderOption && messageSymbols && (
                <AbiValueEditor
                  symbols={messageSymbols}
                  tyIdx={selectedBuilderOption.valueTyIdx}
                  value={argsFormValue}
                  onChange={handleArgsFormChange}
                  addressSuggestions={favoriteAddressSuggestions}
                  disabled={isLoading}
                />
              )}
            </section>
          </div>
        ) : (
          <div className={styles.builderGrid}>
            <section className={styles.builderPanel}>
              <div className={`${styles.panelTitleRow} ${styles.transactionTitleRow}`}>
                <h2 className={styles.panelTitle}>Transaction</h2>
                <InlineAction
                  icon={<RotateCcw size={14} />}
                  label="Reset transaction fields"
                  title="Reset all transaction fields to their default values"
                  onClick={handleReset}
                  disabled={isLoading}
                />
              </div>

              {emulationOptions}
            </section>

            <section className={`${styles.builderPanel} ${styles.payloadPanel}`}>
              <div className={styles.panelTitleRow}>
                <h2 className={styles.panelTitle}>Message</h2>
              </div>

              <textarea
                className={styles.messageInput}
                aria-label="Message BOC"
                value={rawMessage}
                onChange={event => setRawMessage(event.target.value)}
                placeholder="Hex or base64 message BoC"
                spellCheck={false}
                disabled={isLoading}
                rows={8}
              />
            </section>
          </div>
        )}
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

function parseOptionalUint32(value: string): number | undefined {
  const trimmed = value.trim()
  if (!/^\d+$/.test(trimmed)) {
    return undefined
  }
  const parsed = Number(trimmed)
  return Number.isSafeInteger(parsed) && parsed >= 0 && parsed <= MAX_UINT32 ? parsed : undefined
}

async function resolveEmulationUnixTime({
  client,
  mcSeqno,
  mode,
  value,
}: {
  readonly client: TonClient
  readonly mcSeqno: number | undefined
  readonly mode: TimeOverrideMode
  readonly value: string
}): Promise<number | undefined> {
  if (!value.trim()) {
    return undefined
  }

  const parsed = parseOptionalUint32(value)
  if (parsed === undefined) {
    throw new Error(
      mode === "increase"
        ? "Time increase must be a valid uint32 number of seconds"
        : "UNIX timestamp must be a valid uint32",
    )
  }
  if (mode === "timestamp") {
    return parsed
  }

  const blockUnixTime = await loadEmulationBlockUnixTime(client, mcSeqno)
  const result = blockUnixTime + parsed
  if (result > MAX_UINT32) {
    throw new Error("Resulting UNIX timestamp must be a valid uint32")
  }
  return result
}

async function loadEmulationBlockUnixTime(
  client: TonClient,
  mcSeqno: number | undefined,
): Promise<number> {
  const response = await client.getBlocks({
    workchain: -1,
    seqno: mcSeqno,
    limit: 1,
    sort: "desc",
  })
  const block = response.blocks[0]
  const unixTime = block ? Number(block.gen_utime) : Number.NaN
  if (!Number.isSafeInteger(unixTime) || unixTime < 0 || unixTime > MAX_UINT32) {
    throw new Error("Failed to resolve selected masterchain block time")
  }
  return unixTime
}

function formatEmulationUnixTime(value: number): string {
  return new Date(value * 1000).toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
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
    balance: "",
    stateKind: "keep",
    currentStateKind: undefined,
    codeBoc: "",
    storageEnabled: false,
    storageSource: "abi",
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
  const currentStateKind = accountState?.type ?? "uninit"

  return {
    ...entry,
    address,
    abi,
    loadedAddress: address,
    loadState: {type: "ready"},
    balance: fromNano(account?.storage.balance.coins ?? 0n),
    stateKind: entry.stateKind,
    currentStateKind,
    codeBoc,
    storageEnabled: storage.storageEnabled,
    storageSource: storage.storageSource,
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
  "storageEnabled" | "storageSource" | "storageJson" | "storageFormValue"
> {
  if (!dataBoc) {
    return {
      storageEnabled: false,
      storageSource: entry.storageSource,
      storageJson: entry.storageJson,
      storageFormValue: entry.storageFormValue,
    }
  }

  if (abi) {
    try {
      const storageJson = stringifyAbiJson(decodeAbiStorageDataBoc(abi, dataBoc))
      return {
        storageEnabled: entry.storageEnabled,
        storageSource: "abi",
        storageJson,
        storageFormValue: parseAbiJson(storageJson, {}),
      }
    } catch {
      // Fall through to raw BOC when the loaded contract state does not match the active ABI.
    }
  }

  return {
    storageEnabled: entry.storageEnabled,
    storageSource: "raw",
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

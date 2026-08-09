import {useLocation, useNavigate, useParams} from "react-router"
import {useCallback, useEffect, useMemo, useReducer, useRef, useState} from "react"
import type {FC, SetStateAction} from "react"

import {codeLookupHashHex} from "@acton/transaction-ui"
import {Dialog, HighlightedCode, RawDataBlock, TokenAmount} from "@acton/ui"
import {ListChecks, ScrollText, UsersRound} from "lucide-react"
import {Cell} from "@ton/core"

import type {AccountHistorySortOrder, TonClient} from "../api/client"
import type {ExtendedContractABI} from "../api/compilerAbi"
import {sortJettonWalletsByAmount} from "../api/jettonWallets"
import {isAddressSuspended} from "../api/suspendedAccounts"
import type {
  AddressInformation,
  AccountStatesResponse,
  AccountStateTokenInfo,
  JettonMaster,
  JettonMasterMetadata,
  JettonWallet,
  NftItem,
  V3AccountState,
  V3Action,
  V3Metadata,
  V3Multisig,
  V3MultisigOrder,
  V3Transaction,
  V3TransactionListItem,
  VerificationSourceResponse,
} from "../api/types"
import {AccountInfo} from "../components/AccountInfo"
import {ExplorerAddressChip} from "../components/ExplorerAddressChip"
import {ExplorerBreadcrumbs} from "../components/ExplorerBreadcrumbs"
import {
  AccountDetails,
  readAccountHistorySortOrder,
  type ActionTraceLoadMoreState,
  type AccountDetailsTab,
} from "../components/AccountDetails"
import {LockerOverview} from "../components/LockerOverview"
import {JettonOverview} from "../components/JettonOverview"
import {
  MultisigOrderActionsTab,
  MultisigOrdersTab,
  MultisigOverview,
  MultisigSignersTab,
  type MultisigDetailsState,
} from "../components/multisig-details"
import {NftImage} from "../components/NftImage"
import {NftOverview} from "../components/NftOverview"
import {SuspendedAccountOverview} from "../components/SuspendedAccountOverview"
import {VestingOverview} from "../components/VestingOverview"
import {
  NFT_CARD_IMAGE_SOURCE_KEYS,
  NFT_COLLECTION_CARD_IMAGE_SOURCE_KEYS,
  NFT_IMAGE_SOURCE_KEYS,
  TOKEN_IMAGE_SOURCE_KEYS,
  TOKEN_PLACEHOLDER_IMAGE,
  getImageSources,
  replaceBrokenImageWithFallback,
} from "../components/imageFallbacks"
import {mergeAccountDomains, normalizeAddress, toRawAddress} from "../components/utils"
import type {VestingData} from "../components/vestingSchedule"
import {useAddressBook} from "../hooks/useAddressBook"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useNetworkInfo} from "../hooks/useNetworkInfo"
import {useOpenExplorerPath, type ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"
import {useMetadataRegistry} from "../metadata/MetadataRegistryProvider"
import {
  countActionsForTrace,
  mergeAutomaticActionPage,
  mergeStreamedActions,
  type AccountActionPageCursor,
} from "./accountActionPagination"
import {
  getAccountContractTypeByCodeHash,
  hasAccountContractHint,
  hasAccountInterface,
} from "./accountContractTypes"
import styles from "./AccountPage.module.css"

interface AccountPageProps {
  readonly client: TonClient
  readonly enableJettonMint?: boolean
  readonly enableTransactionStreaming?: boolean
  readonly jettonMintPath?: string
  readonly showActonscanLink?: boolean
  readonly tokensLoadMoreLimit?: number
  readonly holdersLoadMoreLimit?: number
}

const INITIAL_TRANSACTION_LIMIT = 20
const REMOTE_TRANSACTION_PAGE_SIZE = 20
const LOCAL_TRANSACTION_PAGE_SIZE = 1000
const ACTION_PAGE_SIZE = 20
const ACTION_TRACE_LOAD_MORE_PAGE_SIZE = 20
const ACCOUNT_TOKENS_INITIAL_LIMIT = 100
const ACCOUNT_TOKENS_LOAD_MORE_LIMIT = 100
const JETTON_HOLDERS_INITIAL_LIMIT = 100
const JETTON_HOLDERS_LOAD_MORE_LIMIT = 100
// The account NFT grid spans one through six columns across its supported widths.
const NFT_CARD_GRID_BATCH_SIZE = 60
const NEW_TRANSACTION_APPEAR_MS = 1400
type AccountTab =
  | "history"
  | "contract"
  | "get-methods"
  | "tokens"
  | "nfts"
  | "items"
  | "holders"
  | "signers"
  | "orders"
  | "actions"

interface AccountTokensState {
  readonly wallets: JettonWallet[]
  readonly isLoading: boolean
  readonly isLoadingMore: boolean
  readonly hasMore: boolean
  readonly loadMoreError?: string
}

interface JettonHoldersState {
  readonly wallets: JettonWallet[]
  readonly loadedAccountKey?: string
  readonly isLoading: boolean
  readonly isLoadingMore: boolean
  readonly hasMore: boolean
  readonly loadMoreError?: string
}

interface NftItemsState {
  readonly items: NftItem[]
  readonly nextOffset: number
  readonly isLoading: boolean
  readonly isLoadingMore: boolean
  readonly hasMore: boolean
  readonly loadMoreError?: string
}

interface AccountLoadIssue {
  readonly title: string
  readonly description: string
  readonly detail: string
  readonly networkLabel: string
}

interface AccountActionHistoryState {
  readonly actions: V3Action[]
  readonly tracesLoadMore: Record<string, ActionTraceLoadMoreState>
}

type AccountActionHistoryStateAction =
  | {readonly type: "set-actions"; readonly update: SetStateAction<V3Action[]>}
  | {
      readonly type: "set-traces-load-more"
      readonly update: SetStateAction<Record<string, ActionTraceLoadMoreState>>
    }
  | {
      readonly type: "merge-streamed-actions"
      readonly actions: readonly V3Action[]
      readonly sortOrder: AccountHistorySortOrder
    }

const INITIAL_ACCOUNT_ACTION_HISTORY_STATE: AccountActionHistoryState = {
  actions: [],
  tracesLoadMore: {},
}

function accountActionHistoryReducer(
  state: AccountActionHistoryState,
  action: AccountActionHistoryStateAction,
): AccountActionHistoryState {
  switch (action.type) {
    case "set-actions":
      return {
        ...state,
        actions: resolveStateUpdate(state.actions, action.update),
      }
    case "set-traces-load-more":
      return {
        ...state,
        tracesLoadMore: resolveStateUpdate(state.tracesLoadMore, action.update),
      }
    case "merge-streamed-actions": {
      const merged = mergeStreamedActions(state.actions, action.actions, action.sortOrder)
      return {
        actions: merged.actions,
        tracesLoadMore: markCollapsedActionTraces(
          state.tracesLoadMore,
          merged.collapsedTraceIds,
          merged.actions,
        ),
      }
    }
    default:
      return state
  }
}

function resolveStateUpdate<T>(current: T, update: SetStateAction<T>): T {
  return typeof update === "function" ? (update as (current: T) => T)(current) : update
}

export const AccountPage: FC<AccountPageProps> = ({
  client,
  enableJettonMint = false,
  enableTransactionStreaming = true,
  jettonMintPath,
  showActonscanLink = false,
  tokensLoadMoreLimit = ACCOUNT_TOKENS_LOAD_MORE_LIMIT,
  holdersLoadMoreLimit = JETTON_HOLDERS_LOAD_MORE_LIMIT,
}) => {
  const {address = ""} = useParams<{address: string}>()
  const navigate = useNavigate()
  const location = useLocation()
  const routes = useExplorerRoutePaths()
  const openPath = useOpenExplorerPath()
  const {addressFormat, network} = useNetworkInfo()
  const metadataRegistry = useMetadataRegistry()
  const {updateDomains} = useAddressBook()
  const [accountState, setAccountState] = useState<AddressInformation | undefined>()
  const [accountStateV3, setAccountStateV3] = useState<V3AccountState | undefined>()
  const [accountSuspendedUntil, setAccountSuspendedUntil] = useState<number | undefined>()
  const [vestingData, setVestingData] = useState<VestingData | undefined>()
  const [multisigDetails, setMultisigDetails] = useState<MultisigDetailsState>({status: "idle"})
  const [multisigReloadKey, setMultisigReloadKey] = useState(0)
  const [hoveredMultisigSignerAddress, setHoveredMultisigSignerAddress] = useState<
    string | undefined
  >()
  const [accountDomain, setAccountDomain] = useState<string | undefined>()
  const [accountDomains, setAccountDomains] = useState<readonly string[]>([])
  const [transactions, setTransactions] = useState<V3TransactionListItem[]>([])
  const [historySortOrder, setHistorySortOrder] = useState<AccountHistorySortOrder>(
    readAccountHistorySortOrder,
  )
  const [{actions, tracesLoadMore: actionTracesLoadMore}, dispatchActionHistory] = useReducer(
    accountActionHistoryReducer,
    INITIAL_ACCOUNT_ACTION_HISTORY_STATE,
  )
  const setActions = useCallback(
    (update: SetStateAction<V3Action[]>) => dispatchActionHistory({type: "set-actions", update}),
    [],
  )
  const setActionTracesLoadMore = useCallback(
    (update: SetStateAction<Record<string, ActionTraceLoadMoreState>>) =>
      dispatchActionHistory({type: "set-traces-load-more", update}),
    [],
  )
  const [actionMetadata, setActionMetadata] = useState<V3Metadata>({})
  const [highlightedTransactionHashes, setHighlightedTransactionHashes] = useState<string[]>([])
  const [transactionsHasMore, setTransactionsHasMore] = useState(false)
  const [transactionsLoadingMore, setTransactionsLoadingMore] = useState(false)
  const [actionsCursor, setActionsCursor] = useState<AccountActionPageCursor>({offset: 0})
  const [actionsHasMore, setActionsHasMore] = useState(false)
  const [actionsLoadingMore, setActionsLoadingMore] = useState(false)
  const [jettonMaster, setJettonMaster] = useState<JettonMaster | undefined>()
  const [jettonWalletAccount, setJettonWalletAccount] = useState<JettonWallet | undefined>()
  const [jettonWalletMaster, setJettonWalletMaster] = useState<JettonMasterMetadata | undefined>()
  const [accountTokensState, setAccountTokensState] = useState<AccountTokensState>({
    wallets: [],
    isLoading: false,
    isLoadingMore: false,
    hasMore: false,
  })
  const [accountTokenInfo, setAccountTokenInfo] = useState<readonly AccountStateTokenInfo[]>([])
  const [currentNftItem, setCurrentNftItem] = useState<NftItem | undefined>()
  const [nftCollectionItemsState, setNftCollectionItemsState] = useState<NftItemsState>({
    items: [],
    nextOffset: 0,
    isLoading: false,
    isLoadingMore: false,
    hasMore: false,
  })
  const [accountNftsState, setAccountNftsState] = useState<NftItemsState>({
    items: [],
    nextOffset: 0,
    isLoading: false,
    isLoadingMore: false,
    hasMore: false,
  })
  const [jettonHoldersState, setJettonHoldersState] = useState<JettonHoldersState>({
    wallets: [],
    isLoading: false,
    isLoadingMore: false,
    hasMore: false,
  })
  const [jettonWalletLoading, setJettonWalletLoading] = useState(false)
  const [transactionsLoading, setTransactionsLoading] = useState(true)
  const [transactionsError, setTransactionsError] = useState<string | undefined>()
  const [actionsLoading, setActionsLoading] = useState(false)
  const [actionsError, setActionsError] = useState<string | undefined>()
  const [accountLoading, setAccountLoading] = useState(true)
  const [accountError, setAccountError] = useState<string | undefined>()
  const [extendedContractAbi, setExtendedContractAbi] = useState<ExtendedContractABI | undefined>()
  const [compilerAbiLoading, setCompilerAbiLoading] = useState(false)
  const [compilerAbiError, setCompilerAbiError] = useState<string | undefined>()
  const [verifiedSource, setVerifiedSource] = useState<VerificationSourceResponse | undefined>()
  const [verifiedSourceLoading, setVerifiedSourceLoading] = useState(false)
  const [jettonMetadataOpen, setJettonMetadataOpen] = useState(false)
  const [additionalDataReadyKey, setAdditionalDataReadyKey] = useState<string | undefined>()
  const activeAccountKeyRef = useRef<string | undefined>(undefined)
  const activeHistoryRequestKeyRef = useRef<string | undefined>(undefined)
  const loadedAccountKeyRef = useRef<string | undefined>(undefined)
  const dnsRequestedAccountKeyRef = useRef<string | undefined>(undefined)
  const transactionHashesRef = useRef<Set<string>>(new Set())
  const isLoadingMoreJettonWalletsRef = useRef(false)
  const isLoadingMoreJettonHoldersRef = useRef(false)
  const isLoadingMoreNftItemsRef = useRef(false)
  const isLoadingMoreNftCollectionItemsRef = useRef(false)
  const jettonWallets = accountTokensState.wallets
  const jettonWalletsLoading = accountTokensState.isLoading
  const jettonWalletsHasMore = accountTokensState.hasMore
  const jettonWalletsLoadingMore = accountTokensState.isLoadingMore
  const holders = jettonHoldersState.wallets
  const holdersLoadedAccountKey = jettonHoldersState.loadedAccountKey
  const holdersLoading = jettonHoldersState.isLoading
  const nftItems = accountNftsState.items
  const nftItemsLoading = accountNftsState.isLoading
  const currentNftCollectionItems = nftCollectionItemsState.items
  const actionTracesLoadMoreWithRemaining = useMemo(
    () => attachRemainingActionCounts(actionTracesLoadMore, transactions),
    [actionTracesLoadMore, transactions],
  )

  const formattedAddress = useMemo(
    () => normalizeAddress(address, addressFormat),
    [address, addressFormat],
  )
  const accountAddressKey = useMemo(() => toRawAddress(formattedAddress), [formattedAddress])
  const accountRequestKey = useMemo(
    () => `${network.id}:${accountAddressKey}`,
    [accountAddressKey, network.id],
  )
  useEffect(() => {
    setHoveredMultisigSignerAddress(undefined)
  }, [accountRequestKey])
  const historyRequestKey = `${accountRequestKey}:${historySortOrder}`
  const activeTab = useMemo<AccountTab>(() => {
    const tab = location.hash.replace("#", "")
    if (tab.startsWith("contract-") || tab.startsWith("abi-")) {
      return "contract"
    }
    return isAccountTab(tab) ? tab : "history"
  }, [location.hash])
  const accountInterfaces = accountStateV3?.interfaces ?? []
  const accountCodeLookupHash = useMemo(() => {
    if (!accountState?.code) return accountStateV3?.code_hash

    try {
      return codeLookupHashHex(Cell.fromBase64(accountState.code))
    } catch {
      return accountStateV3?.code_hash
    }
  }, [accountState?.code, accountStateV3?.code_hash])
  const compilerAbi = extendedContractAbi?.compiler_abi
  const isJettonMasterAccount = hasAccountContractHint(
    accountInterfaces,
    accountTokenInfo,
    "jetton_master",
  )
  const isJettonWalletAccount = hasAccountContractHint(
    accountInterfaces,
    accountTokenInfo,
    "jetton_wallet",
  )
  const isNftItemAccount = hasAccountContractHint(accountInterfaces, accountTokenInfo, "nft_item")
  const isNftCollectionAccount = hasAccountContractHint(
    accountInterfaces,
    accountTokenInfo,
    "nft_collection",
  )
  const isMultisigWalletAccount = hasAccountInterface(accountInterfaces, "multisig_v2")
  const isMultisigOrderAccount = hasAccountInterface(accountInterfaces, "multisig_order_v2")
  const isMultisigAccount = isMultisigWalletAccount || isMultisigOrderAccount
  const usesToncenterApi = client.usesToncenterApiEndpoint()
  const supportsAccountActions = usesToncenterApi && network.supportsActions
  const useTransactionPagination = !usesToncenterApi
  const initialTransactionLimit = usesToncenterApi
    ? INITIAL_TRANSACTION_LIMIT
    : LOCAL_TRANSACTION_PAGE_SIZE
  const transactionPageSize = usesToncenterApi
    ? REMOTE_TRANSACTION_PAGE_SIZE
    : LOCAL_TRANSACTION_PAGE_SIZE

  useEffect(() => {
    let active = true
    setAccountSuspendedUntil(undefined)

    if (!formattedAddress) return

    const load = async () => {
      try {
        const config = await client.getSuspendedAccountsConfig()
        if (!active) return

        setAccountSuspendedUntil(
          isAddressSuspended(config, formattedAddress) ? config.suspendedUntil : undefined,
        )
      } catch (error) {
        console.error("Failed to fetch suspended accounts config", error)
      }
    }

    void load()
    return () => {
      active = false
    }
  }, [client, formattedAddress])

  useEffect(() => {
    const kind = isMultisigWalletAccount ? "wallet" : isMultisigOrderAccount ? "order" : undefined
    if (!kind || !formattedAddress) {
      setMultisigDetails({status: "idle"})
      return
    }

    let active = true
    setMultisigDetails({status: "loading", address: formattedAddress, kind})

    const load = async () => {
      try {
        if (kind === "wallet") {
          const response = await client.getMultisigWallets([formattedAddress], true)
          const wallet: V3Multisig | undefined = response.multisigs[0]
          if (!wallet) {
            throw new Error("Toncenter did not return this multisig wallet.")
          }
          if (active) {
            updateDomains(response.address_book)
            setMultisigDetails({
              status: "success",
              address: formattedAddress,
              kind,
              wallet,
            })
          }
          return
        }

        const response = await client.getMultisigOrders([formattedAddress], true)
        const order: V3MultisigOrder | undefined = response.orders[0]
        if (!order) {
          throw new Error("Toncenter did not return this multisig order.")
        }
        if (active) {
          updateDomains(response.address_book)
          setMultisigDetails({
            status: "success",
            address: formattedAddress,
            kind,
            order,
          })
        }
      } catch (error) {
        if (active) {
          setMultisigDetails({
            status: "error",
            address: formattedAddress,
            kind,
            message: error instanceof Error ? error.message : String(error),
          })
        }
      }
    }

    void load()
    return () => {
      active = false
    }
  }, [
    client,
    formattedAddress,
    isMultisigOrderAccount,
    isMultisigWalletAccount,
    multisigReloadKey,
    updateDomains,
  ])

  useEffect(() => {
    let isActive = true
    const load = () => {
      if (!formattedAddress) {
        activeAccountKeyRef.current = undefined
        activeHistoryRequestKeyRef.current = undefined
        loadedAccountKeyRef.current = undefined
        dnsRequestedAccountKeyRef.current = undefined
        setAdditionalDataReadyKey(undefined)
        setAccountState(undefined)
        setAccountStateV3(undefined)
        setAccountDomain(undefined)
        setAccountDomains([])
        setTransactions([])
        setActions([])
        setActionMetadata({})
        setHighlightedTransactionHashes([])
        transactionHashesRef.current = new Set()
        setTransactionsHasMore(false)
        setTransactionsLoadingMore(false)
        setActionsCursor({offset: 0})
        setActionsHasMore(false)
        setActionsLoadingMore(false)
        setActionTracesLoadMore({})
        setJettonMaster(undefined)
        setJettonWalletAccount(undefined)
        setJettonWalletMaster(undefined)
        setAccountTokensState({
          wallets: [],
          isLoading: false,
          isLoadingMore: false,
          hasMore: false,
        })
        setAccountTokenInfo([])
        setCurrentNftItem(undefined)
        setNftCollectionItemsState({
          items: [],
          nextOffset: 0,
          isLoading: false,
          isLoadingMore: false,
          hasMore: false,
        })
        setAccountNftsState({
          items: [],
          nextOffset: 0,
          isLoading: false,
          isLoadingMore: false,
          hasMore: false,
        })
        setJettonHoldersState({
          wallets: [],
          isLoading: false,
          isLoadingMore: false,
          hasMore: false,
        })
        setJettonWalletLoading(false)
        setTransactionsLoading(false)
        setTransactionsError(undefined)
        setActionsLoading(false)
        setActionsError(undefined)
        setAccountLoading(false)
        setAccountError(undefined)
        return
      }

      const isAddressChange = activeAccountKeyRef.current !== accountRequestKey
      const isHistoryChange = activeHistoryRequestKeyRef.current !== historyRequestKey
      activeAccountKeyRef.current = accountRequestKey
      activeHistoryRequestKeyRef.current = historyRequestKey

      if (isAddressChange) {
        loadedAccountKeyRef.current = undefined
        dnsRequestedAccountKeyRef.current = undefined
        setAdditionalDataReadyKey(undefined)
        setAccountLoading(true)
        setTransactionsLoading(true)
        setActionsLoading(supportsAccountActions)
        setAccountState(undefined)
        setAccountStateV3(undefined)
        setAccountDomain(undefined)
        setAccountDomains([])
        setTransactions([])
        setActions([])
        setActionMetadata({})
        setHighlightedTransactionHashes([])
        transactionHashesRef.current = new Set()
        setTransactionsHasMore(false)
        setTransactionsLoadingMore(false)
        setActionsCursor({offset: 0})
        setActionsHasMore(false)
        setActionsLoadingMore(false)
        setActionTracesLoadMore({})
        setJettonMaster(undefined)
        setJettonWalletAccount(undefined)
        setJettonWalletMaster(undefined)
        setAccountTokensState({
          wallets: [],
          isLoading: false,
          isLoadingMore: false,
          hasMore: false,
        })
        setAccountTokenInfo([])
        setCurrentNftItem(undefined)
        setNftCollectionItemsState({
          items: [],
          nextOffset: 0,
          isLoading: false,
          isLoadingMore: false,
          hasMore: false,
        })
        setAccountNftsState({
          items: [],
          nextOffset: 0,
          isLoading: false,
          isLoadingMore: false,
          hasMore: false,
        })
        setJettonHoldersState({
          wallets: [],
          isLoading: false,
          isLoadingMore: false,
          hasMore: false,
        })
        setJettonWalletLoading(false)
      } else if (isHistoryChange) {
        setTransactionsLoading(true)
        setActionsLoading(supportsAccountActions)
        setTransactions([])
        setActions([])
        setActionMetadata({})
        setHighlightedTransactionHashes([])
        transactionHashesRef.current = new Set()
        setTransactionsHasMore(false)
        setTransactionsLoadingMore(false)
        setActionsCursor({offset: 0})
        setActionsHasMore(false)
        setActionsLoadingMore(false)
        setActionTracesLoadMore({})
      }
      setAccountError(undefined)
      setTransactionsError(undefined)
      setActionsError(undefined)

      const loadAccountState = async () => {
        try {
          const [state, stateV3] = await Promise.all([
            client.getAddressInformation(formattedAddress),
            client.getAccountStates([formattedAddress], false).catch(() => {}),
          ])
          const currentTokenInfo = getAccountTokenInfo(stateV3)
          const currentDomain = getAccountDomain(stateV3)
          if (!isActive) return
          if (stateV3) updateDomains(stateV3.address_book)
          loadedAccountKeyRef.current = accountRequestKey
          setAccountState(state)
          setAccountStateV3(stateV3 ? stateV3.accounts[0] : undefined)
          setAccountDomain(currentDomain)
          setAccountDomains(currentDomain ? [currentDomain] : [])
          setAccountTokenInfo(currentTokenInfo)
        } catch (error) {
          if (!isActive) return
          loadedAccountKeyRef.current = undefined
          setAccountError(error instanceof Error ? error.message : "An error occurred")
          setAccountState(undefined)
          setAccountStateV3(undefined)
          setAccountDomain(undefined)
          setAccountDomains([])
          setTransactions([])
          setActions([])
          setActionMetadata({})
          setJettonMaster(undefined)
          setJettonWalletAccount(undefined)
          setJettonWalletMaster(undefined)
          setAccountTokensState({
            wallets: [],
            isLoading: false,
            isLoadingMore: false,
            hasMore: false,
          })
          setAccountTokenInfo([])
          setCurrentNftItem(undefined)
          setNftCollectionItemsState({
            items: [],
            nextOffset: 0,
            isLoading: false,
            isLoadingMore: false,
            hasMore: false,
          })
          setAccountNftsState({
            items: [],
            nextOffset: 0,
            isLoading: false,
            isLoadingMore: false,
            hasMore: false,
          })
          setJettonHoldersState({
            wallets: [],
            isLoading: false,
            isLoadingMore: false,
            hasMore: false,
          })
          setJettonWalletLoading(false)
          setTransactionsLoading(false)
          setActionsLoading(false)
        } finally {
          if (isActive) setAccountLoading(false)
        }
      }

      const loadTransactions = async () => {
        try {
          const txs = await client.getAccountTransactions(
            formattedAddress,
            initialTransactionLimit,
            0,
            historySortOrder,
          )
          if (!isActive) return
          updateDomains(txs.address_book)
          setTransactions([...txs.transactions])
          transactionHashesRef.current = transactionHashSet(txs.transactions)
          setTransactionsHasMore(txs.transactions.length === initialTransactionLimit)
          setTransactionsError(undefined)
        } catch (error) {
          if (!isActive) return
          console.error("Failed to fetch account transactions", error)
          setTransactions([])
          setHighlightedTransactionHashes([])
          transactionHashesRef.current = new Set()
          setTransactionsHasMore(false)
          setTransactionsError(
            error instanceof Error ? error.message : "Failed to load transactions",
          )
        } finally {
          if (isActive) setTransactionsLoading(false)
        }
      }

      const loadActions = async () => {
        if (!supportsAccountActions) {
          setActions([])
          setActionMetadata({})
          setActionsCursor({offset: 0})
          setActionsHasMore(false)
          setActionsLoadingMore(false)
          setActionTracesLoadMore({})
          setActionsLoading(false)
          setActionsError(undefined)
          return
        }

        try {
          const response = await client.getAccountActions(
            formattedAddress,
            ACTION_PAGE_SIZE,
            0,
            historySortOrder,
          )
          if (!isActive) return
          updateDomains(response.address_book)
          const merged = mergeAutomaticActionPage(
            [],
            response.actions,
            {offset: 0},
            historySortOrder,
            ACTION_PAGE_SIZE,
          )
          setActions(merged.actions)
          setActionMetadata(response.metadata)
          setActionsCursor(merged.cursor)
          setActionsHasMore(merged.hasMore)
          setActionTracesLoadMore(current =>
            markCollapsedActionTraces(current, merged.collapsedTraceIds, merged.actions),
          )
          setActionsError(undefined)
        } catch (error) {
          if (!isActive) return
          console.error("Failed to fetch account actions", error)
          setActions([])
          setActionMetadata({})
          setActionsCursor({offset: 0})
          setActionsHasMore(false)
          setActionTracesLoadMore({})
          setActionsError(error instanceof Error ? error.message : "Failed to load actions")
        } finally {
          if (isActive) setActionsLoading(false)
        }
      }

      if (isAddressChange || !isHistoryChange) {
        void loadAccountState()
      }
      void loadTransactions()
      void loadActions()
    }

    load()
    return () => {
      isActive = false
    }
  }, [
    accountRequestKey,
    client,
    historyRequestKey,
    historySortOrder,
    initialTransactionLimit,
    setActions,
    setActionTracesLoadMore,
    supportsAccountActions,
    updateDomains,
  ])

  useEffect(() => {
    if (
      !formattedAddress ||
      loadedAccountKeyRef.current !== accountRequestKey ||
      accountLoading ||
      transactionsLoading ||
      actionsLoading
    ) {
      return
    }
    setAdditionalDataReadyKey(accountRequestKey)
  }, [accountLoading, accountRequestKey, actionsLoading, formattedAddress, transactionsLoading])

  useEffect(() => {
    if (
      !formattedAddress ||
      additionalDataReadyKey !== accountRequestKey ||
      dnsRequestedAccountKeyRef.current === accountRequestKey
    ) {
      return
    }

    let isActive = true
    dnsRequestedAccountKeyRef.current = accountRequestKey
    void client
      .getWalletDnsNames(formattedAddress)
      .then(domains => {
        if (!isActive) return
        const nextDomains = mergeAccountDomains(accountDomain, domains)
        setAccountDomain(nextDomains[0])
        setAccountDomains(nextDomains)
      })
      .catch(() => {
        // The singular domain from accountStates remains available as a fallback.
      })

    return () => {
      isActive = false
    }
  }, [accountDomain, accountRequestKey, additionalDataReadyKey, client, formattedAddress])

  const loadMoreTransactions = async () => {
    if (
      !formattedAddress ||
      transactionsLoadingMore ||
      transactionsLoading ||
      !transactionsHasMore
    ) {
      return
    }

    setTransactionsLoadingMore(true)
    setTransactionsError(undefined)
    try {
      const txs = await client.getAccountTransactions(
        formattedAddress,
        transactionPageSize,
        transactions.length,
        historySortOrder,
      )
      updateDomains(txs.address_book)
      transactionHashesRef.current = transactionHashSet([...transactions, ...txs.transactions])
      setTransactions(current => appendUniqueTransactions(current, txs.transactions))
      setTransactionsHasMore(txs.transactions.length === transactionPageSize)
    } catch (error) {
      console.error("Failed to load more account transactions", error)
      setTransactionsError(error instanceof Error ? error.message : "Failed to load transactions")
    } finally {
      setTransactionsLoadingMore(false)
    }
  }

  const loadMoreActions = async () => {
    if (
      !formattedAddress ||
      !supportsAccountActions ||
      actionsLoadingMore ||
      actionsLoading ||
      Object.values(actionTracesLoadMore).some(trace => trace.loading) ||
      !actionsHasMore
    ) {
      return
    }

    const cursor = actionsCursor
    setActionsLoadingMore(true)
    setActionsError(undefined)
    try {
      const response = await client.getAccountActions(
        formattedAddress,
        ACTION_PAGE_SIZE,
        cursor.offset,
        historySortOrder,
        {startLt: cursor.startLt, endLt: cursor.endLt},
      )
      updateDomains(response.address_book)
      const merged = mergeAutomaticActionPage(
        actions,
        response.actions,
        cursor,
        historySortOrder,
        ACTION_PAGE_SIZE,
      )
      setActions(merged.actions)
      setActionMetadata(current => ({...current, ...response.metadata}))
      setActionsCursor(merged.cursor)
      setActionsHasMore(merged.hasMore)
      setActionTracesLoadMore(current =>
        markCollapsedActionTraces(current, merged.collapsedTraceIds, merged.actions),
      )
    } catch (error) {
      console.error("Failed to load more account actions", error)
      setActionsError(error instanceof Error ? error.message : "Failed to load actions")
    } finally {
      setActionsLoadingMore(false)
    }
  }

  const loadMoreActionTrace = async (traceId: string) => {
    if (!formattedAddress || !supportsAccountActions || actionsLoading || actionsLoadingMore) {
      return
    }

    const traceState = actionTracesLoadMore[traceId]
    if (!traceState?.hasMore || traceState.loading) {
      return
    }

    const offset = countActionsForTrace(actions, traceId)
    const requestKey = historyRequestKey
    setActionTracesLoadMore(current => ({
      ...current,
      [traceId]: {...traceState, loading: true, error: undefined},
    }))

    try {
      const response = await client.getAccountActions(
        formattedAddress,
        ACTION_TRACE_LOAD_MORE_PAGE_SIZE + 1,
        offset,
        historySortOrder,
        {traceId},
      )
      if (activeHistoryRequestKeyRef.current !== requestKey) return

      updateDomains(response.address_book)
      const page = response.actions.slice(0, ACTION_TRACE_LOAD_MORE_PAGE_SIZE)
      const nextActions = appendUniqueActions(actions, page)
      setActions(nextActions)
      setActionMetadata(current => ({...current, ...response.metadata}))
      setActionTracesLoadMore(current => ({
        ...current,
        [traceId]: {
          loadedCount: countActionsForTrace(nextActions, traceId),
          loadCount: ACTION_TRACE_LOAD_MORE_PAGE_SIZE,
          hasMore: response.actions.length > ACTION_TRACE_LOAD_MORE_PAGE_SIZE,
          loading: false,
        },
      }))
    } catch (error) {
      if (activeHistoryRequestKeyRef.current !== requestKey) return
      console.error("Failed to load more actions for transaction", error)
      setActionTracesLoadMore(current => ({
        ...current,
        [traceId]: {
          ...(current[traceId] ?? traceState),
          loading: false,
          error: error instanceof Error ? error.message : "Failed to load actions",
        },
      }))
    }
  }

  const handleTransactionClick = (hash: string, event?: ExplorerNavigationClickEvent) => {
    openPath(routes.transactionPath(hash), event)
  }

  useEffect(() => {
    let isActive = true

    const loadCompilerAbi = async () => {
      if (!accountCodeLookupHash) {
        setExtendedContractAbi(undefined)
        setCompilerAbiLoading(false)
        setCompilerAbiError(undefined)
        return
      }

      setExtendedContractAbi(undefined)
      setCompilerAbiLoading(true)
      setCompilerAbiError(undefined)

      try {
        const abis = await metadataRegistry.getCompilerAbis([accountCodeLookupHash])
        if (!isActive) return
        setExtendedContractAbi(abis[accountCodeLookupHash] ?? undefined)
        setCompilerAbiLoading(false)
      } catch (error) {
        if (!isActive) return
        setExtendedContractAbi(undefined)
        setCompilerAbiLoading(false)
        setCompilerAbiError(error instanceof Error ? error.message : "Failed to load compiler ABI")
      }
    }

    void loadCompilerAbi()
    return () => {
      isActive = false
    }
  }, [accountCodeLookupHash, metadataRegistry])

  useEffect(() => {
    let isActive = true

    const loadVerifiedSource = async () => {
      if (!accountCodeLookupHash) {
        setVerifiedSource(undefined)
        setVerifiedSourceLoading(false)
        return
      }

      setVerifiedSource(undefined)
      setVerifiedSourceLoading(true)

      try {
        const source = await metadataRegistry.getSource({
          codeHash: accountCodeLookupHash,
        })
        if (!isActive) return
        setVerifiedSource(source.verified && source.bundle ? source : undefined)
      } catch (error) {
        if (!isActive) return
        console.debug("Failed to fetch verified source", error)
        setVerifiedSource(undefined)
      } finally {
        if (isActive) setVerifiedSourceLoading(false)
      }
    }

    void loadVerifiedSource()
    return () => {
      isActive = false
    }
  }, [accountCodeLookupHash, metadataRegistry])

  useEffect(() => {
    if (!formattedAddress || !enableTransactionStreaming) {
      return
    }

    let isActive = true
    const unsubscribe = client.subscribeAccountHistory(formattedAddress, {
      onTransactions: event => {
        if (event.finality === "pending") {
          return
        }

        const newHashes = collectNewTransactionHashes(
          event.transactions,
          transactionHashesRef.current,
        )
        if (newHashes.length > 0) {
          const newHashSet = new Set(newHashes)
          setHighlightedTransactionHashes(current => [...new Set([...current, ...newHashes])])
          globalThis.setTimeout(() => {
            setHighlightedTransactionHashes(current =>
              current.filter(hash => !newHashSet.has(hash)),
            )
          }, NEW_TRANSACTION_APPEAR_MS)
        }

        setTransactions(current => prependUniqueTransactions(event.transactions, current))
        transactionHashesRef.current = new Set([...newHashes, ...transactionHashesRef.current])
        setTransactionsLoading(false)
        setTransactionsError(undefined)
      },
      onActions: supportsAccountActions
        ? event => {
            if (event.finality === "pending") {
              return
            }

            if (event.address_book) {
              updateDomains(event.address_book)
            }
            if (event.metadata) {
              setActionMetadata(current => ({...current, ...event.metadata}))
            }
            dispatchActionHistory({
              type: "merge-streamed-actions",
              actions: event.actions,
              sortOrder: historySortOrder,
            })
            setActionsLoading(false)
            setActionsError(undefined)
          }
        : undefined,
      onError: error => {
        if (isActive) {
          console.debug("Account transaction stream closed", error)
        }
      },
    })

    return () => {
      isActive = false
      unsubscribe()
    }
  }, [
    accountAddressKey,
    client,
    enableTransactionStreaming,
    historySortOrder,
    supportsAccountActions,
    updateDomains,
  ])

  useEffect(() => {
    setJettonMetadataOpen(false)
  }, [accountAddressKey])

  useEffect(() => {
    let isActive = true

    const loadJettonMaster = async () => {
      if (!formattedAddress || !isJettonMasterAccount) {
        setJettonMaster(undefined)
        return
      }

      try {
        const masters = await client.getJettonMasters([formattedAddress])
        if (!isActive) return
        setJettonMaster(masters[0])
      } catch (error) {
        console.error("Failed to fetch jetton master", error)
      }
    }

    void loadJettonMaster()
    return () => {
      isActive = false
    }
  }, [accountAddressKey, client, isJettonMasterAccount])

  useEffect(() => {
    let isActive = true

    const loadJettonWallet = async () => {
      if (!formattedAddress || !isJettonWalletAccount) {
        setJettonWalletAccount(undefined)
        setJettonWalletMaster(undefined)
        setJettonWalletLoading(false)
        return
      }

      setJettonWalletLoading(true)
      try {
        const currentWallets = await client.getJettonWalletsByAddress([formattedAddress])
        const currentWallet = currentWallets[0]
        if (!isActive) return
        setJettonWalletAccount(currentWallet)
        setJettonWalletMaster(currentWallet?.master)

        if (!currentWallet || currentWallet.master) {
          return
        }

        const currentWalletMasters = await client.getJettonMasters([currentWallet.jetton])
        if (!isActive) return
        setJettonWalletMaster(currentWalletMasters[0])
      } catch (error) {
        if (!isActive) return
        console.error("Failed to fetch jetton wallet", error)
        setJettonWalletAccount(undefined)
        setJettonWalletMaster(undefined)
      } finally {
        if (isActive) setJettonWalletLoading(false)
      }
    }

    void loadJettonWallet()
    return () => {
      isActive = false
    }
  }, [accountAddressKey, client, isJettonWalletAccount])

  useEffect(() => {
    let isActive = true

    const loadJettonWallets = async () => {
      if (!formattedAddress) {
        return
      }

      setAccountTokensState(current => ({
        ...current,
        isLoading: true,
        isLoadingMore: false,
        hasMore: false,
        loadMoreError: undefined,
      }))
      try {
        const wallets = await client.getJettonWallets([formattedAddress], undefined, {
          limit: ACCOUNT_TOKENS_INITIAL_LIMIT,
        })
        if (!isActive) return
        setAccountTokensState({
          wallets: sortJettonWalletsByAmount(wallets),
          isLoading: false,
          isLoadingMore: false,
          hasMore: wallets.length === ACCOUNT_TOKENS_INITIAL_LIMIT,
        })
      } catch (error) {
        console.error("Failed to fetch account jetton wallets", error)
        if (isActive) {
          setAccountTokensState({
            wallets: [],
            isLoading: false,
            isLoadingMore: false,
            hasMore: false,
          })
        }
      }
    }

    void loadJettonWallets()
    return () => {
      isActive = false
    }
  }, [accountAddressKey, client])

  const loadMoreJettonWallets = useCallback(() => {
    const offset = accountTokensState.wallets.length
    if (
      !formattedAddress ||
      accountTokensState.isLoading ||
      !accountTokensState.hasMore ||
      isLoadingMoreJettonWalletsRef.current
    ) {
      return
    }

    const requestAccountKey = accountRequestKey
    isLoadingMoreJettonWalletsRef.current = true
    setAccountTokensState(current => ({
      ...current,
      isLoadingMore: true,
      loadMoreError: undefined,
    }))

    void client
      .getJettonWallets([formattedAddress], undefined, {
        limit: tokensLoadMoreLimit,
        offset,
      })
      .then(wallets => {
        setAccountTokensState(current => {
          if (
            activeAccountKeyRef.current !== requestAccountKey ||
            current.isLoading ||
            current.wallets.length !== offset
          ) {
            return current
          }

          return {
            ...current,
            wallets: [...current.wallets, ...wallets],
            isLoadingMore: false,
            hasMore: wallets.length === tokensLoadMoreLimit,
          }
        })
      })
      .catch(error => {
        setAccountTokensState(current => {
          if (
            activeAccountKeyRef.current !== requestAccountKey ||
            current.isLoading ||
            current.wallets.length !== offset
          ) {
            return current
          }

          return {
            ...current,
            isLoadingMore: false,
            loadMoreError:
              error instanceof Error ? error.message : "Failed to load more account tokens",
          }
        })
      })
      .finally(() => {
        isLoadingMoreJettonWalletsRef.current = false
      })
  }, [
    accountRequestKey,
    accountTokensState.hasMore,
    accountTokensState.isLoading,
    accountTokensState.wallets.length,
    client,
    formattedAddress,
    tokensLoadMoreLimit,
  ])

  useEffect(() => {
    let isActive = true

    const loadNftItem = async () => {
      if (!formattedAddress || !isNftItemAccount) {
        setCurrentNftItem(undefined)
        return
      }

      try {
        const items = await client.getNftItems({
          address: [formattedAddress],
          limit: 1,
        })
        if (!isActive) return
        setCurrentNftItem(items[0])
      } catch (error) {
        console.error("Failed to fetch NFT item", error)
      }
    }

    void loadNftItem()
    return () => {
      isActive = false
    }
  }, [accountAddressKey, client, isNftItemAccount])

  useEffect(() => {
    let isActive = true

    const loadNftCollectionItems = async () => {
      if (!formattedAddress || !isNftCollectionAccount) {
        setNftCollectionItemsState({
          items: [],
          nextOffset: 0,
          isLoading: false,
          isLoadingMore: false,
          hasMore: false,
        })
        return
      }

      setNftCollectionItemsState(current => ({
        ...current,
        isLoading: true,
        isLoadingMore: false,
        hasMore: false,
        loadMoreError: undefined,
      }))
      try {
        const page = await client.getNftItemsPage({
          collection_address: [formattedAddress],
          limit: NFT_CARD_GRID_BATCH_SIZE,
          sortByLastTransactionLt: true,
        })
        if (!isActive) return
        setNftCollectionItemsState({
          items: page.items,
          nextOffset: page.rawItemCount,
          isLoading: false,
          isLoadingMore: false,
          hasMore: page.rawItemCount === NFT_CARD_GRID_BATCH_SIZE,
        })
      } catch (error) {
        console.error("Failed to fetch NFT collection items", error)
        if (isActive) {
          setNftCollectionItemsState({
            items: [],
            nextOffset: 0,
            isLoading: false,
            isLoadingMore: false,
            hasMore: false,
          })
        }
      }
    }

    void loadNftCollectionItems()
    return () => {
      isActive = false
    }
  }, [accountAddressKey, client, isNftCollectionAccount])

  const loadMoreNftCollectionItems = useCallback(() => {
    const offset = nftCollectionItemsState.nextOffset
    if (
      !formattedAddress ||
      nftCollectionItemsState.isLoading ||
      !nftCollectionItemsState.hasMore ||
      isLoadingMoreNftCollectionItemsRef.current
    ) {
      return
    }

    const requestAccountKey = accountRequestKey
    isLoadingMoreNftCollectionItemsRef.current = true
    setNftCollectionItemsState(current => ({
      ...current,
      isLoadingMore: true,
      loadMoreError: undefined,
    }))

    void client
      .getNftItemsPage({
        collection_address: [formattedAddress],
        limit: NFT_CARD_GRID_BATCH_SIZE,
        offset,
        sortByLastTransactionLt: true,
      })
      .then(page => {
        setNftCollectionItemsState(current => {
          if (
            activeAccountKeyRef.current !== requestAccountKey ||
            current.isLoading ||
            current.nextOffset !== offset
          ) {
            return current
          }

          return {
            ...current,
            items: [...current.items, ...page.items],
            nextOffset: offset + page.rawItemCount,
            isLoadingMore: false,
            hasMore: page.rawItemCount === NFT_CARD_GRID_BATCH_SIZE,
          }
        })
      })
      .catch(error => {
        setNftCollectionItemsState(current => {
          if (
            activeAccountKeyRef.current !== requestAccountKey ||
            current.isLoading ||
            current.nextOffset !== offset
          ) {
            return current
          }

          return {
            ...current,
            isLoadingMore: false,
            loadMoreError:
              error instanceof Error ? error.message : "Failed to load more collection items",
          }
        })
      })
      .finally(() => {
        isLoadingMoreNftCollectionItemsRef.current = false
      })
  }, [
    accountRequestKey,
    client,
    formattedAddress,
    nftCollectionItemsState.hasMore,
    nftCollectionItemsState.isLoading,
    nftCollectionItemsState.nextOffset,
  ])

  useEffect(() => {
    let isActive = true

    const loadNftItems = async () => {
      if (!formattedAddress) {
        setAccountNftsState({
          items: [],
          nextOffset: 0,
          isLoading: false,
          isLoadingMore: false,
          hasMore: false,
        })
        return
      }

      setAccountNftsState(current => ({
        ...current,
        isLoading: true,
        isLoadingMore: false,
        hasMore: false,
        loadMoreError: undefined,
      }))
      try {
        const page = await client.getNftItemsPage({
          owner_address: [formattedAddress],
          limit: NFT_CARD_GRID_BATCH_SIZE,
          sortByLastTransactionLt: true,
        })
        if (!isActive) return
        setAccountNftsState({
          items: page.items,
          nextOffset: page.rawItemCount,
          isLoading: false,
          isLoadingMore: false,
          hasMore: page.rawItemCount === NFT_CARD_GRID_BATCH_SIZE,
        })
      } catch (error) {
        console.error("Failed to fetch account NFTs", error)
        if (isActive) {
          setAccountNftsState({
            items: [],
            nextOffset: 0,
            isLoading: false,
            isLoadingMore: false,
            hasMore: false,
          })
        }
      }
    }

    void loadNftItems()
    return () => {
      isActive = false
    }
  }, [accountAddressKey, client])

  const loadMoreNftItems = useCallback(() => {
    const offset = accountNftsState.nextOffset
    if (
      !formattedAddress ||
      accountNftsState.isLoading ||
      !accountNftsState.hasMore ||
      isLoadingMoreNftItemsRef.current
    ) {
      return
    }

    const requestAccountKey = accountRequestKey
    isLoadingMoreNftItemsRef.current = true
    setAccountNftsState(current => ({
      ...current,
      isLoadingMore: true,
      loadMoreError: undefined,
    }))

    void client
      .getNftItemsPage({
        owner_address: [formattedAddress],
        limit: NFT_CARD_GRID_BATCH_SIZE,
        offset,
        sortByLastTransactionLt: true,
      })
      .then(page => {
        setAccountNftsState(current => {
          if (
            activeAccountKeyRef.current !== requestAccountKey ||
            current.isLoading ||
            current.nextOffset !== offset
          ) {
            return current
          }

          return {
            ...current,
            items: [...current.items, ...page.items],
            nextOffset: offset + page.rawItemCount,
            isLoadingMore: false,
            hasMore: page.rawItemCount === NFT_CARD_GRID_BATCH_SIZE,
          }
        })
      })
      .catch(error => {
        setAccountNftsState(current => {
          if (
            activeAccountKeyRef.current !== requestAccountKey ||
            current.isLoading ||
            current.nextOffset !== offset
          ) {
            return current
          }

          return {
            ...current,
            isLoadingMore: false,
            loadMoreError:
              error instanceof Error ? error.message : "Failed to load more account NFTs",
          }
        })
      })
      .finally(() => {
        isLoadingMoreNftItemsRef.current = false
      })
  }, [
    accountNftsState.hasMore,
    accountNftsState.isLoading,
    accountNftsState.nextOffset,
    accountRequestKey,
    client,
    formattedAddress,
  ])

  useEffect(() => {
    let isActive = true

    const loadHolders = async () => {
      if (
        !formattedAddress ||
        activeTab !== "holders" ||
        !isJettonMasterAccount ||
        holdersLoadedAccountKey === accountRequestKey
      ) {
        return
      }

      setJettonHoldersState(current => ({
        ...current,
        isLoading: true,
        isLoadingMore: false,
        hasMore: false,
        loadMoreError: undefined,
      }))
      try {
        const masterHolders = await client.getJettonWallets(undefined, [formattedAddress], {
          limit: JETTON_HOLDERS_INITIAL_LIMIT,
          sort: "desc",
        })
        if (!isActive) return
        setJettonHoldersState({
          wallets: sortJettonWalletsByAmount(masterHolders),
          loadedAccountKey: accountRequestKey,
          isLoading: false,
          isLoadingMore: false,
          hasMore: masterHolders.length === JETTON_HOLDERS_INITIAL_LIMIT,
        })
      } catch (error) {
        console.error("Failed to fetch jetton holders", error)
        if (isActive) {
          setJettonHoldersState({
            wallets: [],
            loadedAccountKey: accountRequestKey,
            isLoading: false,
            isLoadingMore: false,
            hasMore: false,
          })
        }
      }
    }

    void loadHolders()
    return () => {
      isActive = false
    }
  }, [
    accountAddressKey,
    accountRequestKey,
    activeTab,
    client,
    holdersLoadedAccountKey,
    isJettonMasterAccount,
  ])

  const loadMoreJettonHolders = useCallback(() => {
    const offset = jettonHoldersState.wallets.length
    if (
      !formattedAddress ||
      jettonHoldersState.loadedAccountKey !== accountRequestKey ||
      jettonHoldersState.isLoading ||
      !jettonHoldersState.hasMore ||
      isLoadingMoreJettonHoldersRef.current
    ) {
      return
    }

    const requestAccountKey = accountRequestKey
    isLoadingMoreJettonHoldersRef.current = true
    setJettonHoldersState(current => ({
      ...current,
      isLoadingMore: true,
      loadMoreError: undefined,
    }))

    void client
      .getJettonWallets(undefined, [formattedAddress], {
        limit: holdersLoadMoreLimit,
        offset,
        sort: "desc",
      })
      .then(wallets => {
        setJettonHoldersState(current => {
          if (
            activeAccountKeyRef.current !== requestAccountKey ||
            current.loadedAccountKey !== requestAccountKey ||
            current.isLoading ||
            current.wallets.length !== offset
          ) {
            return current
          }

          return {
            ...current,
            wallets: [...current.wallets, ...wallets],
            isLoadingMore: false,
            hasMore: wallets.length === holdersLoadMoreLimit,
          }
        })
      })
      .catch(error => {
        setJettonHoldersState(current => {
          if (
            activeAccountKeyRef.current !== requestAccountKey ||
            current.loadedAccountKey !== requestAccountKey ||
            current.isLoading ||
            current.wallets.length !== offset
          ) {
            return current
          }

          return {
            ...current,
            isLoadingMore: false,
            loadMoreError:
              error instanceof Error ? error.message : "Failed to load more jetton holders",
          }
        })
      })
      .finally(() => {
        isLoadingMoreJettonHoldersRef.current = false
      })
  }, [
    accountRequestKey,
    client,
    formattedAddress,
    holdersLoadMoreLimit,
    jettonHoldersState.hasMore,
    jettonHoldersState.isLoading,
    jettonHoldersState.loadedAccountKey,
    jettonHoldersState.wallets.length,
  ])

  const holdersPending =
    holdersLoading ||
    (activeTab === "holders" &&
      (accountLoading || (isJettonMasterAccount && holdersLoadedAccountKey !== accountRequestKey)))

  const handleSearch = useCallback(
    (addr: string, event?: ExplorerNavigationClickEvent) => {
      openPath(addr ? routes.addressPath(addr) : routes.rootPath, event)
    },
    [openPath, routes],
  )
  const handleMultisigOrderClick = useCallback(
    (addr: string, event?: ExplorerNavigationClickEvent) => {
      openPath(`${routes.addressPath(addr)}#actions`, event)
    },
    [openPath, routes],
  )

  const handleTabChange = (tab: string) => {
    if (tab === "holders" && holdersLoadedAccountKey !== accountRequestKey) {
      setJettonHoldersState(current => ({...current, isLoading: true}))
    }

    const hash = tab === "contract" ? "contract-storage" : tab
    if (location.hash === `#${hash}`) return
    void navigate(`${location.pathname}#${hash}`)
  }

  useEffect(() => {
    if (isMultisigOrderAccount && !location.hash) {
      void navigate(`${location.pathname}#actions`, {replace: true})
    }
  }, [isMultisigOrderAccount, location.hash, location.pathname, navigate])

  const tokenInfo = jettonMaster ?? jettonWalletMaster
  const tokenSymbol = tokenInfo?.jetton_content.symbol
  const tokenName = tokenInfo?.jetton_content.name || "Unknown Jetton"
  const tokenDecimals = tokenInfo?.jetton_content.decimals
  const tokenImageSources = getImageSources(tokenInfo?.jetton_content, TOKEN_IMAGE_SOURCE_KEYS)
  const tokenImage = tokenImageSources[0] ?? TOKEN_PLACEHOLDER_IMAGE
  const jettonMasterAdminAddress = jettonMaster?.admin_address ?? undefined
  const jettonMetadataJson = jettonMaster
    ? JSON.stringify(
        {
          address: toRawAddress(jettonMaster.address),
          ...jettonMaster.jetton_content,
        },
        undefined,
        2,
      )
    : undefined
  const nftItemTokenInfo = accountTokenInfo.find(info => info.type === "nft_items")
  const nftCollectionTokenInfo = accountTokenInfo.find(info => info.type === "nft_collections")
  const nftItemName =
    tokenInfoString(nftItemTokenInfo, "name") ||
    contentString(currentNftItem?.content, "name") ||
    (currentNftItem ? `NFT #${currentNftItem.index}` : undefined)
  const nftItemDescription =
    tokenInfoString(nftItemTokenInfo, "description") ||
    contentString(currentNftItem?.content, "description")
  const nftItemImageSources = [
    ...getImageSources(nftItemTokenInfo, NFT_IMAGE_SOURCE_KEYS),
    ...getImageSources(currentNftItem?.content, NFT_IMAGE_SOURCE_KEYS),
  ]
  const nftItemCollectionName =
    tokenInfoString(nftItemTokenInfo, "collection_name") ||
    contentString(currentNftItem?.content, "collection_name")
  const nftItemIsScam = nftItemTokenInfo?.is_scam === true || currentNftItem?.is_scam === true
  const nftItemMetadataJson = currentNftItem
    ? JSON.stringify(
        {
          address: toRawAddress(currentNftItem.address),
          index: currentNftItem.index,
          owner_address: currentNftItem.owner_address,
          collection_address: currentNftItem.collection_address,
          ...currentNftItem.content,
        },
        undefined,
        2,
      )
    : undefined
  const nftItemOwnerAddress = currentNftItem?.owner_address
  const nftItemCollectionAddress = currentNftItem?.collection_address
  const activeMetadataJson = jettonMaster ? jettonMetadataJson : nftItemMetadataJson
  const activeMetadataTitle = jettonMaster ? tokenName : (nftItemName ?? "NFT item")
  const activeMetadataImageSources = jettonMaster
    ? tokenImageSources
    : currentNftItem
      ? nftItemImageSources
      : []
  const activeMetadataImage = activeMetadataImageSources[0]
  const collectionSample = currentNftCollectionItems[0]
  const nftCollectionName =
    tokenInfoString(nftCollectionTokenInfo, "name") ||
    contentString(collectionSample?.content, "collection_name") ||
    (nftCollectionTokenInfo || currentNftCollectionItems.length > 0 ? "NFT Collection" : undefined)
  const nftCollectionDescription =
    tokenInfoString(nftCollectionTokenInfo, "description") ||
    contentString(collectionSample?.content, "collection_description")
  const nftCollectionImageSources = [
    ...getImageSources(nftCollectionTokenInfo, NFT_CARD_IMAGE_SOURCE_KEYS),
    ...getImageSources(collectionSample?.content, NFT_COLLECTION_CARD_IMAGE_SOURCE_KEYS),
  ]
  const nftCollectionIsNsfw = nftCollectionTokenInfo?.is_nsfw === true
  const nftCollectionIsScam = nftCollectionTokenInfo?.is_scam === true
  const collectiblePreviews = nftItems.slice(0, 8).map(item => {
    const imageSources = getImageSources(item.content, NFT_IMAGE_SOURCE_KEYS)
    return {
      address: item.address,
      image: imageSources[0] ?? TOKEN_PLACEHOLDER_IMAGE,
      imageSources,
      blurred: item.is_scam === true,
      collectionName: contentString(item.content, "collection_name"),
      name:
        contentString(item.content, "name") ||
        contentString(item.content, "collection_name") ||
        `NFT #${item.index}`,
    }
  })
  const accountLoadIssue = useMemo(
    () =>
      accountError
        ? getAccountLoadIssue({
            error: accountError,
            networkLabel: network.label,
          })
        : undefined,
    [accountError, network.label],
  )
  const accountUnavailable =
    accountLoadIssue !== undefined && !accountLoading && accountState === undefined

  const showAccountHeader = accountLoading || accountState !== undefined || accountUnavailable

  const codeHashContractType = getAccountContractTypeByCodeHash(accountCodeLookupHash)

  const isLockerAccount = codeHashContractType === "locker"
  const isVestingAccount = codeHashContractType === "vesting"
  const isScheduleAccount = isLockerAccount || isVestingAccount
  const accountSuspended = accountSuspendedUntil !== undefined

  const currentMultisigDetails: MultisigDetailsState =
    multisigDetails.status !== "idle" && multisigDetails.address !== formattedAddress
      ? {
          status: "loading",
          address: formattedAddress,
          kind: isMultisigOrderAccount ? "order" : "wallet",
        }
      : multisigDetails

  const multisigOrder =
    currentMultisigDetails.status === "success" && currentMultisigDetails.kind === "order"
      ? currentMultisigDetails.order
      : undefined

  const hasHeaderContextCard =
    accountState !== undefined &&
    (tokenInfo !== undefined ||
      currentNftItem !== undefined ||
      nftCollectionName !== undefined ||
      isScheduleAccount ||
      isMultisigAccount ||
      accountSuspended)

  const topSectionClassName = hasHeaderContextCard
    ? isScheduleAccount || isMultisigAccount || accountSuspended
      ? `${styles.topSection} ${styles.topSectionEqual}`
      : styles.topSection
    : `${styles.topSection} ${styles.topSectionSingle}`

  const accountInfoDetails =
    isVestingAccount && vestingData
      ? [
          {
            key: "vesting-owner",
            label: "Owner",
            value: (
              <ExplorerAddressChip
                address={vestingData.ownerAddress}
                onAddressClick={handleSearch}
                variant="plain"
              />
            ),
          },
          {
            key: "vesting-sender",
            label: "Sender",
            value: (
              <ExplorerAddressChip
                address={vestingData.vestingSenderAddress}
                onAddressClick={handleSearch}
                variant="plain"
              />
            ),
          },
        ]
      : multisigOrder
        ? [
            {
              key: "multisig-wallet",
              label: "Multisig wallet",
              value: (
                <ExplorerAddressChip
                  address={multisigOrder.multisig_address}
                  onAddressClick={handleSearch}
                  variant="plain"
                />
              ),
            },
          ]
        : undefined

  const multisigTabs = useMemo<readonly AccountDetailsTab[]>(() => {
    if (isMultisigWalletAccount) {
      return [
        {
          id: "signers",
          label: "Signers",
          icon: <UsersRound size={18} />,
          content: (
            <MultisigSignersTab
              state={currentMultisigDetails}
              approvalActions={actions}
              onAddressClick={handleSearch}
              hoveredSignerAddress={hoveredMultisigSignerAddress}
              onSignerHoverChange={setHoveredMultisigSignerAddress}
            />
          ),
        },
        {
          id: "orders",
          label: "Orders",
          icon: <ScrollText size={18} />,
          content: (
            <MultisigOrdersTab
              state={currentMultisigDetails}
              onAddressClick={handleSearch}
              onOrderClick={handleMultisigOrderClick}
            />
          ),
        },
      ]
    }
    if (isMultisigOrderAccount) {
      return [
        {
          id: "signers",
          label: "Signers",
          icon: <UsersRound size={18} />,
          content: (
            <MultisigSignersTab
              state={currentMultisigDetails}
              approvalActions={actions}
              onAddressClick={handleSearch}
              hoveredSignerAddress={hoveredMultisigSignerAddress}
              onSignerHoverChange={setHoveredMultisigSignerAddress}
            />
          ),
        },
        {
          id: "actions",
          label: "Actions",
          icon: <ListChecks size={18} />,
          content: (
            <MultisigOrderActionsTab state={currentMultisigDetails} onAddressClick={handleSearch} />
          ),
        },
      ]
    }
    return []
  }, [
    currentMultisigDetails,
    actions,
    handleMultisigOrderClick,
    handleSearch,
    hoveredMultisigSignerAddress,
    isMultisigOrderAccount,
    isMultisigWalletAccount,
  ])

  return (
    <div className={styles.container}>
      {formattedAddress && (
        <>
          <ExplorerBreadcrumbs
            items={[
              {
                label: formattedAddress,
                isAddress: true,
                copy: {
                  value: formattedAddress,
                  label: "Copy account address",
                  copiedLabel: "Account address copied",
                },
              },
            ]}
          />
          {showAccountHeader && (
            <div className={topSectionClassName}>
              {accountUnavailable && accountLoadIssue ? (
                <AccountIssueCard issue={accountLoadIssue} />
              ) : (
                <AccountInfo
                  address={formattedAddress}
                  domain={accountDomain}
                  domains={accountDomains}
                  state={accountState}
                  extendedContractAbi={extendedContractAbi}
                  contractInterfaces={
                    Array.isArray(accountStateV3?.interfaces)
                      ? accountStateV3.interfaces
                      : undefined
                  }
                  jettonWallets={jettonWallets}
                  accountLoading={accountLoading}
                  assetsLoading={accountLoading || jettonWalletsLoading}
                  amount={
                    jettonWalletAccount && jettonWalletMaster ? (
                      <TokenAmount
                        decimals={jettonWalletMaster.jetton_content.decimals}
                        symbol={tokenSymbol}
                        useGrouping
                        value={jettonWalletAccount.balance}
                      />
                    ) : undefined
                  }
                  amountLoading={isJettonWalletAccount && jettonWalletLoading}
                  details={accountInfoDetails}
                  client={client}
                  onMoreAssetsClick={() => handleTabChange("tokens")}
                  collectiblesCount={nftItems.length}
                  collectiblePreviews={collectiblePreviews}
                  collectiblesLoading={accountLoading || nftItemsLoading}
                  onCollectiblesClick={() => handleTabChange("nfts")}
                  hasContextCard={hasHeaderContextCard}
                  showActonscanLink={showActonscanLink}
                />
              )}
              {hasHeaderContextCard && (
                <div className={styles.contextColumn}>
                  {accountSuspendedUntil !== undefined && (
                    <SuspendedAccountOverview suspendedUntil={accountSuspendedUntil} />
                  )}
                  {isLockerAccount && <LockerOverview address={formattedAddress} client={client} />}
                  {isVestingAccount && (
                    <VestingOverview
                      address={formattedAddress}
                      client={client}
                      onDataChange={setVestingData}
                    />
                  )}
                  {isMultisigAccount && (
                    <MultisigOverview
                      state={currentMultisigDetails}
                      approvalActions={actions}
                      onRetry={() => setMultisigReloadKey(key => key + 1)}
                      hoveredSignerAddress={hoveredMultisigSignerAddress}
                      onSignerHoverChange={setHoveredMultisigSignerAddress}
                    />
                  )}
                  {accountState !== undefined && tokenInfo !== undefined && (
                    <JettonOverview
                      name={tokenName}
                      symbol={tokenSymbol}
                      image={tokenImage}
                      imageSources={tokenImageSources}
                      decimals={tokenDecimals}
                      totalSupply={jettonMaster?.total_supply}
                      masterAddress={
                        jettonMaster === undefined && jettonWalletMaster !== undefined
                          ? jettonWalletAccount?.jetton
                          : undefined
                      }
                      holderAddress={
                        jettonMaster === undefined && jettonWalletMaster !== undefined
                          ? jettonWalletAccount?.owner
                          : undefined
                      }
                      onAddressClick={handleSearch}
                      onMetadataClick={
                        jettonMaster === undefined ? undefined : () => setJettonMetadataOpen(true)
                      }
                      onMint={
                        enableJettonMint === true &&
                        jettonMaster?.mintable === true &&
                        jettonMintPath !== undefined &&
                        jettonMintPath.length > 0
                          ? () =>
                              void navigate(
                                `${jettonMintPath}?jetton=${encodeURIComponent(
                                  toRawAddress(jettonMaster.address),
                                )}`,
                              )
                          : undefined
                      }
                    />
                  )}
                  {accountState !== undefined && currentNftItem !== undefined && (
                    <NftOverview
                      kind="item"
                      name={nftItemName ?? `NFT #${currentNftItem.index}`}
                      description={nftItemDescription}
                      imageSources={nftItemImageSources}
                      isScam={nftItemIsScam}
                      ownerAddress={nftItemOwnerAddress}
                      collectionAddress={nftItemCollectionAddress}
                      collectionName={nftItemCollectionName}
                      index={currentNftItem.index}
                      onAddressClick={handleSearch}
                      onMetadataClick={() => setJettonMetadataOpen(true)}
                      onNsfw={() => setCurrentNftItem(undefined)}
                    />
                  )}
                  {accountState !== undefined &&
                    nftCollectionName !== undefined &&
                    currentNftItem === undefined &&
                    !nftCollectionIsNsfw && (
                      <NftOverview
                        kind="collection"
                        name={nftCollectionName}
                        description={nftCollectionDescription}
                        imageSources={nftCollectionImageSources}
                        isScam={nftCollectionIsScam}
                        latestItemAddress={collectionSample?.address}
                        onAddressClick={handleSearch}
                      />
                    )}
                </div>
              )}
            </div>
          )}
          <AccountDetails
            transactions={transactions}
            actions={actions}
            actionMetadata={actionMetadata}
            highlightedTransactionHashes={highlightedTransactionHashes}
            accountState={accountState}
            compilerAbi={compilerAbi}
            compilerAbiLoading={compilerAbiLoading}
            compilerAbiError={compilerAbiError}
            verifiedSource={verifiedSource}
            verifiedSourceLoading={verifiedSourceLoading}
            ownerAddress={formattedAddress}
            jettonWallets={jettonWallets}
            nftItems={nftItems}
            collectionItems={currentNftCollectionItems}
            jettonMaster={jettonMaster}
            holders={holders}
            tokensLoading={jettonWalletsLoading}
            tokensHasMore={jettonWalletsHasMore}
            tokensLoadingMore={jettonWalletsLoadingMore}
            tokensLoadMoreError={accountTokensState.loadMoreError}
            nftsLoading={nftItemsLoading}
            nftsHasMore={accountNftsState.hasMore}
            nftsLoadingMore={accountNftsState.isLoadingMore}
            nftsLoadMoreError={accountNftsState.loadMoreError}
            collectionItemsLoading={nftCollectionItemsState.isLoading}
            collectionItemsHasMore={nftCollectionItemsState.hasMore}
            collectionItemsLoadingMore={nftCollectionItemsState.isLoadingMore}
            collectionItemsLoadMoreError={nftCollectionItemsState.loadMoreError}
            holdersLoading={holdersPending}
            holdersHasMore={jettonHoldersState.hasMore}
            holdersLoadingMore={jettonHoldersState.isLoadingMore}
            holdersLoadMoreError={jettonHoldersState.loadMoreError}
            transactionsLoading={transactionsLoading}
            transactionsError={accountUnavailable ? undefined : transactionsError}
            transactionsHasMore={transactionsHasMore}
            transactionsLoadingMore={transactionsLoadingMore}
            transactionsPaginated={useTransactionPagination}
            actionsSupported={supportsAccountActions}
            actionsLoading={actionsLoading}
            actionsError={accountUnavailable ? undefined : actionsError}
            actionsHasMore={actionsHasMore}
            actionsLoadingMore={actionsLoadingMore}
            actionTracesLoadMore={actionTracesLoadMoreWithRemaining}
            accountLoading={accountLoading}
            showHoldersTab={isJettonMasterAccount}
            showItemsTab={isNftCollectionAccount}
            customTabs={multisigTabs}
            client={client}
            onAddressClick={handleSearch}
            onTransactionClick={handleTransactionClick}
            onLoadMoreTokens={loadMoreJettonWallets}
            onLoadMoreNfts={loadMoreNftItems}
            onLoadMoreCollectionItems={loadMoreNftCollectionItems}
            onLoadMoreHolders={loadMoreJettonHolders}
            onLoadMoreTransactions={loadMoreTransactions}
            onLoadMoreActions={loadMoreActions}
            onLoadMoreActionTrace={loadMoreActionTrace}
            historySortOrder={historySortOrder}
            onHistorySortOrderChange={setHistorySortOrder}
            activeTabHash={activeTab}
            onTabChange={handleTabChange}
          />
          {activeMetadataJson && (
            <Dialog
              open={jettonMetadataOpen}
              onOpenChange={setJettonMetadataOpen}
              title="Metadata"
              closeLabel="Close metadata"
              maxWidth="42rem"
              contentClassName={styles.metadataDialogContent}
            >
              <div className={styles.metadataOverview}>
                {activeMetadataImage &&
                  (currentNftItem ? (
                    <NftImage
                      sources={activeMetadataImageSources}
                      alt=""
                      className={`${styles.metadataTokenImage} ${styles.metadataNftImage}`}
                      blurredClassName={styles.blurredImage}
                      collectionName={nftItemCollectionName}
                      blurred={nftItemIsScam}
                      onNsfw={() => setCurrentNftItem(undefined)}
                    />
                  ) : (
                    <img
                      src={activeMetadataImage}
                      alt=""
                      className={styles.metadataTokenImage}
                      onError={event =>
                        replaceBrokenImageWithFallback(event, activeMetadataImageSources)
                      }
                    />
                  ))}
                <div className={styles.metadataIdentity}>
                  <h3 className={styles.metadataTokenTitle}>{activeMetadataTitle}</h3>
                  {(jettonMaster?.jetton_content.description || nftItemDescription) && (
                    <p className={styles.metadataDescription}>
                      {jettonMaster?.jetton_content.description || nftItemDescription}
                    </p>
                  )}
                </div>
              </div>
              <dl className={styles.metadataSummary}>
                <div className={styles.metadataRow}>
                  <dt className={styles.metadataLabel}>Address</dt>
                  <dd className={styles.metadataValue}>
                    <ExplorerAddressChip address={formattedAddress} variant="plain" />
                  </dd>
                </div>
                {jettonMaster && (
                  <>
                    <div className={styles.metadataRow}>
                      <dt className={styles.metadataLabel}>Owner</dt>
                      <dd className={styles.metadataValue}>
                        {jettonMasterAdminAddress ? (
                          <ExplorerAddressChip
                            address={jettonMasterAdminAddress}
                            onAddressClick={handleSearch}
                            variant="plain"
                          />
                        ) : (
                          "None"
                        )}
                      </dd>
                    </div>
                    {jettonMaster && (
                      <div className={styles.metadataRow}>
                        <dt className={styles.metadataLabel}>Max supply</dt>
                        <dd className={styles.metadataValue}>
                          <TokenAmount
                            decimals={tokenDecimals}
                            symbol={tokenSymbol}
                            useGrouping
                            value={jettonMaster.total_supply}
                          />
                        </dd>
                      </div>
                    )}
                    <div className={styles.metadataRow}>
                      <dt className={styles.metadataLabel}>Mintable</dt>
                      <dd className={styles.metadataValue}>{String(jettonMaster.mintable)}</dd>
                    </div>
                  </>
                )}
                {currentNftItem && (
                  <>
                    <div className={styles.metadataRow}>
                      <dt className={styles.metadataLabel}>Owner</dt>
                      <dd className={styles.metadataValue}>
                        {nftItemOwnerAddress ? (
                          <ExplorerAddressChip
                            address={nftItemOwnerAddress}
                            onAddressClick={handleSearch}
                            variant="plain"
                          />
                        ) : (
                          "No owner"
                        )}
                      </dd>
                    </div>
                    <div className={styles.metadataRow}>
                      <dt className={styles.metadataLabel}>Collection</dt>
                      <dd className={styles.metadataValue}>
                        {nftItemCollectionAddress ? (
                          <ExplorerAddressChip
                            address={nftItemCollectionAddress}
                            onAddressClick={handleSearch}
                            variant="plain"
                          />
                        ) : (
                          "Standalone"
                        )}
                      </dd>
                    </div>
                    <div className={styles.metadataRow}>
                      <dt className={styles.metadataLabel}>Index</dt>
                      <dd className={styles.metadataValue}>#{currentNftItem.index}</dd>
                    </div>
                  </>
                )}
              </dl>
              <RawDataBlock
                title="Raw metadata"
                value={activeMetadataJson}
                copyLabel="metadata JSON"
                maxHeight="18rem"
                customContent={
                  <HighlightedCode value={activeMetadataJson} language="json" maxHeight="18rem" />
                }
              />
            </Dialog>
          )}
        </>
      )}

      {!accountState && !accountLoading && !accountError && formattedAddress && (
        <div className={styles.empty}>No data found for this address.</div>
      )}
    </div>
  )
}

interface AccountIssueCardProps {
  readonly issue: AccountLoadIssue
}

const AccountIssueCard: FC<AccountIssueCardProps> = ({issue}) => (
  <section className={styles.accountIssue} aria-live="polite">
    <div className={styles.accountIssueContent}>
      <div className={styles.accountIssueEyebrow}>{issue.networkLabel}</div>
      <h1 className={styles.accountIssueTitle}>{issue.title}</h1>
      <p className={styles.accountIssueDescription}>{issue.detail}</p>
      <div className={styles.accountIssueHint}>
        <span className={styles.accountIssueHintLabel}>Hint</span>
        <span className={styles.accountIssueHintText}>{issue.description}</span>
      </div>
    </div>
  </section>
)

function getAccountTokenInfo(
  stateV3: AccountStatesResponse | void,
): readonly AccountStateTokenInfo[] {
  if (!stateV3) return []
  const currentAccount = stateV3.accounts[0]
  return currentAccount ? (stateV3.metadata[currentAccount.address]?.token_info ?? []) : []
}

function getAccountDomain(stateV3: AccountStatesResponse | void): string | undefined {
  if (!stateV3) return undefined
  const currentAccount = stateV3.accounts[0]
  const domain = currentAccount ? stateV3.address_book[currentAccount.address]?.domain : undefined
  return domain?.trim() || undefined
}

function tokenInfoString(info: AccountStateTokenInfo | undefined, key: string): string | undefined {
  const value = info?.[key]
  return typeof value === "string" && value.length > 0 ? value : undefined
}

function contentString(
  content: Record<string, unknown> | undefined,
  key: string,
): string | undefined {
  const value = content?.[key]
  return typeof value === "string" && value.length > 0 ? value : undefined
}

function getAccountLoadIssue({
  error,
  networkLabel,
}: {
  readonly error: string
  readonly networkLabel: string
}): AccountLoadIssue {
  const normalizedError = error.trim() || "Unknown error"
  const lowercaseError = normalizedError.toLowerCase()

  if (
    lowercaseError.includes("failed to fetch") ||
    lowercaseError.includes("networkerror") ||
    lowercaseError.includes("load failed")
  ) {
    return {
      title: "Network request failed",
      description:
        "The selected network did not respond or blocked browser requests. Check that the V2/V3 endpoints are reachable and CORS is enabled.",
      detail: normalizedError,
      networkLabel,
    }
  }

  if (lowercaseError.includes("unauthorized") || lowercaseError.includes("401")) {
    return {
      title: "Authentication failed",
      description:
        "The selected network rejected the request. Check the API key configured for this network.",
      detail: normalizedError,
      networkLabel,
    }
  }

  return {
    title: "Unable to load account",
    description: "The selected network returned an error while loading this account.",
    detail: normalizedError,
    networkLabel,
  }
}

function isAccountTab(value: string): value is AccountTab {
  return (
    value === "history" ||
    value === "contract" ||
    value === "get-methods" ||
    value === "tokens" ||
    value === "nfts" ||
    value === "items" ||
    value === "holders" ||
    value === "signers" ||
    value === "orders" ||
    value === "actions"
  )
}

function transactionHashSet(
  transactions: readonly Pick<V3TransactionListItem, "hash">[],
): Set<string> {
  return new Set(transactions.map(transaction => transaction.hash).filter(Boolean))
}

function collectNewTransactionHashes(
  transactions: readonly Pick<V3TransactionListItem, "hash">[],
  knownHashes: ReadonlySet<string>,
): string[] {
  const nextHashes: string[] = []
  const seen = new Set(knownHashes)

  for (const transaction of transactions) {
    if (!transaction.hash || seen.has(transaction.hash)) {
      continue
    }
    seen.add(transaction.hash)
    nextHashes.push(transaction.hash)
  }

  return nextHashes
}

function appendUniqueTransactions(
  current: readonly V3TransactionListItem[],
  next: readonly V3TransactionListItem[],
): V3TransactionListItem[] {
  const seen = new Set(current.map(transaction => transaction.hash).filter(Boolean))
  const uniqueNext = next.filter(transaction => {
    if (!transaction.hash || seen.has(transaction.hash)) {
      return false
    }
    seen.add(transaction.hash)
    return true
  })
  return [...current, ...uniqueNext]
}

function appendUniqueActions(current: readonly V3Action[], next: readonly V3Action[]): V3Action[] {
  const seen = new Set(current.map(action => action.action_id))
  const uniqueNext = next.filter(action => {
    if (seen.has(action.action_id)) {
      return false
    }
    seen.add(action.action_id)
    return true
  })
  return [...current, ...uniqueNext]
}

function markCollapsedActionTraces(
  current: Readonly<Record<string, ActionTraceLoadMoreState>>,
  traceIds: readonly string[],
  actions: readonly V3Action[],
): Record<string, ActionTraceLoadMoreState> {
  if (traceIds.length === 0) {
    return {...current}
  }

  const next = {...current}
  for (const traceId of traceIds) {
    const existing = current[traceId]
    next[traceId] = {
      loadedCount: countActionsForTrace(actions, traceId),
      loadCount: ACTION_TRACE_LOAD_MORE_PAGE_SIZE,
      hasMore: true,
      loading: existing?.loading ?? false,
      error: existing?.error,
    }
  }
  return next
}

function attachRemainingActionCounts(
  states: Readonly<Record<string, ActionTraceLoadMoreState>>,
  transactions: readonly V3TransactionListItem[],
): Record<string, ActionTraceLoadMoreState> {
  const totalActionsByTrace = new Map<string, number>()
  for (const transaction of transactions) {
    const traceId = transaction.trace_id?.trim() || transaction.hash?.trim()
    const totalActions = transaction.description.action?.tot_actions
    if (traceId && totalActions !== undefined && totalActions >= 0) {
      totalActionsByTrace.set(traceId, totalActions)
    }
  }

  return Object.fromEntries(
    Object.entries(states).map(([traceId, state]) => {
      const totalActions = totalActionsByTrace.get(traceId)
      const remainingCount =
        totalActions === undefined ? undefined : Math.max(0, totalActions - state.loadedCount)
      return [
        traceId,
        {
          ...state,
          remainingCount,
          loadCount:
            remainingCount === undefined
              ? ACTION_TRACE_LOAD_MORE_PAGE_SIZE
              : Math.min(ACTION_TRACE_LOAD_MORE_PAGE_SIZE, remainingCount),
        },
      ]
    }),
  )
}

function prependUniqueTransactions(
  next: readonly V3Transaction[],
  current: readonly V3TransactionListItem[],
): V3TransactionListItem[] {
  const seen = new Set<string>()
  const uniqueNext: V3TransactionListItem[] = []

  for (const transaction of next) {
    if (!transaction.hash || seen.has(transaction.hash)) {
      continue
    }
    seen.add(transaction.hash)
    uniqueNext.push(transaction)
  }

  const currentWithoutDuplicates = current.filter(transaction => {
    if (!transaction.hash || seen.has(transaction.hash)) {
      return false
    }
    seen.add(transaction.hash)
    return true
  })

  return [...uniqueNext, ...currentWithoutDuplicates]
}

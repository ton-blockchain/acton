import {Check, Copy, X} from "lucide-react"
import type {FC, ReactNode} from "react"
import {useEffect, useMemo, useRef, useState} from "react"
import {useLocation, useNavigate, useParams} from "react-router-dom"

import type {TonClient} from "../api/client"
import type {ExtendedContractABI} from "../api/compilerAbi"
import type {
  AccountStatesResponse,
  AccountStateTokenInfo,
  AddressInformation,
  JettonMaster,
  JettonMasterMetadata,
  JettonWallet,
  NftItem,
  V3AccountState,
  V3Action,
  V3Metadata,
  V3Transaction,
  V3TransactionListItem,
  VerificationSourceResponse,
} from "../api/types"
import {AccountDetails} from "../components/AccountDetails"
import {AccountInfo} from "../components/AccountInfo"
import {AddressLabel} from "../components/AddressLabel"
import {Breadcrumbs} from "../components/Breadcrumbs"
import {
  getImageSources,
  NFT_COLLECTION_IMAGE_SOURCE_KEYS,
  NFT_IMAGE_SOURCE_KEYS,
  replaceBrokenImageWithFallback,
  TOKEN_IMAGE_SOURCE_KEYS,
  TOKEN_PLACEHOLDER_IMAGE,
} from "../components/imageFallbacks"
import {normalizeAddress, toRawAddress} from "../components/utils"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useNetworkInfo} from "../hooks/useNetworkInfo"
import {type ExplorerNavigationClickEvent, useOpenExplorerPath} from "../hooks/useOpenExplorerPath"
import {useMetadataRegistry} from "../metadata/MetadataRegistryProvider"

import styles from "./AccountPage.module.css"

interface AccountPageProps {
  readonly client: TonClient
}

const INITIAL_TRANSACTION_LIMIT = 20
const REMOTE_TRANSACTION_PAGE_SIZE = 20
const LOCAL_TRANSACTION_PAGE_SIZE = 1000
const ACTION_PAGE_SIZE = 20
const NEW_TRANSACTION_APPEAR_MS = 1400
type AccountTab = "history" | "contract" | "tokens" | "nfts" | "holders"

interface AccountLoadIssue {
  readonly title: string
  readonly description: string
  readonly detail: string
  readonly networkLabel: string
}

export const AccountPage: FC<AccountPageProps> = ({client}) => {
  const {address = ""} = useParams<{address: string}>()
  const navigate = useNavigate()
  const location = useLocation()
  const routes = useExplorerRoutePaths()
  const openPath = useOpenExplorerPath()
  const {addressFormat, network} = useNetworkInfo()
  const metadataRegistry = useMetadataRegistry()
  const [accountState, setAccountState] = useState<AddressInformation | undefined>()
  const [accountStateV3, setAccountStateV3] = useState<V3AccountState | undefined>()
  const [transactions, setTransactions] = useState<V3TransactionListItem[]>([])
  const [actions, setActions] = useState<V3Action[]>([])
  const [actionMetadata, setActionMetadata] = useState<V3Metadata>({})
  const [highlightedTransactionHashes, setHighlightedTransactionHashes] = useState<string[]>([])
  const [transactionsHasMore, setTransactionsHasMore] = useState(false)
  const [transactionsLoadingMore, setTransactionsLoadingMore] = useState(false)
  const [actionsOffset, setActionsOffset] = useState(0)
  const [actionsHasMore, setActionsHasMore] = useState(false)
  const [actionsLoadingMore, setActionsLoadingMore] = useState(false)
  const [jettonMaster, setJettonMaster] = useState<JettonMaster | undefined>()
  const [jettonWalletAccount, setJettonWalletAccount] = useState<JettonWallet | undefined>()
  const [jettonWalletMaster, setJettonWalletMaster] = useState<JettonMasterMetadata | undefined>()
  const [jettonWallets, setJettonWallets] = useState<JettonWallet[]>([])
  const [accountTokenInfo, setAccountTokenInfo] = useState<readonly AccountStateTokenInfo[]>([])
  const [currentNftItem, setCurrentNftItem] = useState<NftItem | undefined>()
  const [currentNftCollectionItems, setCurrentNftCollectionItems] = useState<NftItem[]>([])
  const [nftItems, setNftItems] = useState<NftItem[]>([])
  const [holders, setHolders] = useState<JettonWallet[]>([])
  const [jettonWalletsLoading, setJettonWalletsLoading] = useState(false)
  const [jettonWalletLoading, setJettonWalletLoading] = useState(false)
  const [nftItemsLoading, setNftItemsLoading] = useState(false)
  const [holdersLoading, setHoldersLoading] = useState(false)
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
  const [jettonMetadataCopied, setJettonMetadataCopied] = useState(false)
  const activeAccountKeyRef = useRef<string | undefined>(undefined)
  const transactionHashesRef = useRef<Set<string>>(new Set())

  const formattedAddress = useMemo(
    () => normalizeAddress(address, addressFormat),
    [address, addressFormat],
  )
  const accountAddressKey = useMemo(() => toRawAddress(formattedAddress), [formattedAddress])
  const accountRequestKey = useMemo(
    () => `${network.id}:${accountAddressKey}`,
    [accountAddressKey, network.id],
  )
  const activeTab = useMemo<AccountTab>(() => {
    const tab = location.hash.replace("#", "")
    if (tab.startsWith("contract-")) {
      return "contract"
    }
    return isAccountTab(tab) ? tab : "history"
  }, [location.hash])
  const accountInterfaces = accountStateV3?.interfaces ?? []
  const accountCodeHash = accountStateV3?.code_hash
  const compilerAbi = extendedContractAbi?.compiler_abi
  const isJettonMasterAccount = hasAccountInterface(accountInterfaces, "jetton_master")
  const isJettonWalletAccount = hasAccountInterface(accountInterfaces, "jetton_wallet")
  const isNftItemAccount = hasAccountInterface(accountInterfaces, "nft_item")
  const isNftCollectionAccount = hasAccountInterface(accountInterfaces, "nft_collection")
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
    let isActive = true
    const load = () => {
      if (!formattedAddress) {
        activeAccountKeyRef.current = undefined
        setAccountState(undefined)
        setAccountStateV3(undefined)
        setTransactions([])
        setActions([])
        setActionMetadata({})
        setHighlightedTransactionHashes([])
        transactionHashesRef.current = new Set()
        setTransactionsHasMore(false)
        setTransactionsLoadingMore(false)
        setActionsOffset(0)
        setActionsHasMore(false)
        setActionsLoadingMore(false)
        setJettonMaster(undefined)
        setJettonWalletAccount(undefined)
        setJettonWalletMaster(undefined)
        setJettonWallets([])
        setAccountTokenInfo([])
        setCurrentNftItem(undefined)
        setCurrentNftCollectionItems([])
        setNftItems([])
        setHolders([])
        setJettonWalletsLoading(false)
        setJettonWalletLoading(false)
        setNftItemsLoading(false)
        setHoldersLoading(false)
        setTransactionsLoading(false)
        setTransactionsError(undefined)
        setActionsLoading(false)
        setActionsError(undefined)
        setAccountLoading(false)
        setAccountError(undefined)
        return
      }

      const isAddressChange = activeAccountKeyRef.current !== accountRequestKey
      activeAccountKeyRef.current = accountRequestKey

      if (isAddressChange) {
        setAccountLoading(true)
        setTransactionsLoading(true)
        setActionsLoading(supportsAccountActions)
        setAccountState(undefined)
        setAccountStateV3(undefined)
        setTransactions([])
        setActions([])
        setActionMetadata({})
        setHighlightedTransactionHashes([])
        transactionHashesRef.current = new Set()
        setTransactionsHasMore(false)
        setTransactionsLoadingMore(false)
        setActionsOffset(0)
        setActionsHasMore(false)
        setActionsLoadingMore(false)
        setJettonMaster(undefined)
        setJettonWalletAccount(undefined)
        setJettonWalletMaster(undefined)
        setJettonWallets([])
        setAccountTokenInfo([])
        setCurrentNftItem(undefined)
        setCurrentNftCollectionItems([])
        setNftItems([])
        setHolders([])
        setJettonWalletsLoading(false)
        setJettonWalletLoading(false)
        setNftItemsLoading(false)
        setHoldersLoading(false)
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
          if (!isActive) return
          setAccountState(state)
          setAccountStateV3(stateV3 ? stateV3.accounts[0] : undefined)
          setAccountTokenInfo(currentTokenInfo)
        } catch (error) {
          if (!isActive) return
          setAccountError(error instanceof Error ? error.message : "An error occurred")
          setAccountState(undefined)
          setAccountStateV3(undefined)
          setTransactions([])
          setActions([])
          setActionMetadata({})
          setJettonMaster(undefined)
          setJettonWalletAccount(undefined)
          setJettonWalletMaster(undefined)
          setJettonWallets([])
          setAccountTokenInfo([])
          setCurrentNftItem(undefined)
          setCurrentNftCollectionItems([])
          setNftItems([])
          setHolders([])
          setJettonWalletsLoading(false)
          setJettonWalletLoading(false)
          setNftItemsLoading(false)
          setHoldersLoading(false)
          setTransactionsLoading(false)
          setActionsLoading(false)
        } finally {
          if (isActive) setAccountLoading(false)
        }
      }

      const loadTransactions = async () => {
        try {
          const txs = await client.getAccountTransactions(formattedAddress, initialTransactionLimit)
          if (!isActive) return
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
          setActionsOffset(0)
          setActionsHasMore(false)
          setActionsLoadingMore(false)
          setActionsLoading(false)
          setActionsError(undefined)
          return
        }

        try {
          const response = await client.getAccountActions(formattedAddress, ACTION_PAGE_SIZE)
          if (!isActive) return
          setActions([...response.actions])
          setActionMetadata(response.metadata)
          setActionsOffset(response.actions.length)
          setActionsHasMore(response.actions.length === ACTION_PAGE_SIZE)
          setActionsError(undefined)
        } catch (error) {
          if (!isActive) return
          console.error("Failed to fetch account actions", error)
          setActions([])
          setActionMetadata({})
          setActionsOffset(0)
          setActionsHasMore(false)
          setActionsError(error instanceof Error ? error.message : "Failed to load actions")
        } finally {
          if (isActive) setActionsLoading(false)
        }
      }

      void loadAccountState()
      void loadTransactions()
      void loadActions()
    }

    load()
    return () => {
      isActive = false
    }
  }, [accountRequestKey, client, initialTransactionLimit, supportsAccountActions])

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
      )
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
      !actionsHasMore
    ) {
      return
    }

    const offset = actionsOffset
    setActionsLoadingMore(true)
    setActionsError(undefined)
    try {
      const response = await client.getAccountActions(formattedAddress, ACTION_PAGE_SIZE, offset)
      setActions(current => [...current, ...response.actions])
      setActionMetadata(current => ({...current, ...response.metadata}))
      setActionsOffset(current => current + response.actions.length)
      setActionsHasMore(response.actions.length === ACTION_PAGE_SIZE)
    } catch (error) {
      console.error("Failed to load more account actions", error)
      setActionsError(error instanceof Error ? error.message : "Failed to load actions")
    } finally {
      setActionsLoadingMore(false)
    }
  }

  const handleTransactionClick = (hash: string, event?: ExplorerNavigationClickEvent) => {
    openPath(routes.transactionPath(hash), event)
  }

  useEffect(() => {
    let isActive = true

    const loadCompilerAbi = async () => {
      if (!accountCodeHash) {
        setExtendedContractAbi(undefined)
        setCompilerAbiLoading(false)
        setCompilerAbiError(undefined)
        return
      }

      setExtendedContractAbi(undefined)
      setCompilerAbiLoading(true)
      setCompilerAbiError(undefined)

      try {
        const abis = await metadataRegistry.getCompilerAbis([accountCodeHash])
        if (!isActive) return
        setExtendedContractAbi(abis[accountCodeHash] ?? undefined)
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
  }, [accountCodeHash, metadataRegistry])

  useEffect(() => {
    let isActive = true

    const loadVerifiedSource = async () => {
      if (!accountCodeHash) {
        setVerifiedSource(undefined)
        setVerifiedSourceLoading(false)
        return
      }

      setVerifiedSource(undefined)
      setVerifiedSourceLoading(true)

      try {
        const source = await metadataRegistry.getSource({
          codeHash: accountCodeHash,
        })
        if (!isActive) return
        setVerifiedSource(source.verified && source.bundles.length > 0 ? source : undefined)
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
  }, [accountCodeHash, metadataRegistry])

  useEffect(() => {
    if (!formattedAddress) {
      return
    }

    let isActive = true
    const unsubscribe = client.subscribeAccountTransactions(formattedAddress, {
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
  }, [accountAddressKey, client])

  useEffect(() => {
    setJettonMetadataOpen(false)
    setJettonMetadataCopied(false)
  }, [accountAddressKey])

  useEffect(() => {
    if (!jettonMetadataCopied) {
      return
    }

    const timer = setTimeout(() => setJettonMetadataCopied(false), 1600)
    return () => clearTimeout(timer)
  }, [jettonMetadataCopied])

  useEffect(() => {
    if (!jettonMetadataOpen) {
      return
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setJettonMetadataOpen(false)
      }
    }

    document.addEventListener("keydown", handleKeyDown)
    return () => document.removeEventListener("keydown", handleKeyDown)
  }, [jettonMetadataOpen])

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

      setJettonWalletsLoading(true)
      try {
        const wallets = await client.getJettonWallets([formattedAddress])
        if (!isActive) return
        setJettonWallets(sortJettonWalletsByAmount(wallets))
      } catch (error) {
        console.error("Failed to fetch account jetton wallets", error)
      } finally {
        if (isActive) setJettonWalletsLoading(false)
      }
    }

    void loadJettonWallets()
    return () => {
      isActive = false
    }
  }, [accountAddressKey, client])

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
        setCurrentNftCollectionItems([])
        return
      }

      try {
        const items = await client.getNftItems({
          collection_address: [formattedAddress],
          limit: 100,
          sortByLastTransactionLt: true,
        })
        if (!isActive) return
        setCurrentNftCollectionItems(items)
      } catch (error) {
        console.error("Failed to fetch NFT collection items", error)
      }
    }

    void loadNftCollectionItems()
    return () => {
      isActive = false
    }
  }, [accountAddressKey, client, isNftCollectionAccount])

  useEffect(() => {
    let isActive = true

    const loadNftItems = async () => {
      if (!formattedAddress) {
        setNftItems([])
        setNftItemsLoading(false)
        return
      }

      setNftItemsLoading(true)
      try {
        const nfts = await client.getNftItems({
          owner_address: [formattedAddress],
          limit: 100,
          sortByLastTransactionLt: true,
        })
        if (!isActive) return
        setNftItems(nfts)
      } catch (error) {
        console.error("Failed to fetch account NFTs", error)
      } finally {
        if (isActive) setNftItemsLoading(false)
      }
    }

    void loadNftItems()
    return () => {
      isActive = false
    }
  }, [accountAddressKey, client])

  useEffect(() => {
    let isActive = true

    const loadHolders = async () => {
      if (!formattedAddress || activeTab !== "holders" || !isJettonMasterAccount) {
        return
      }

      setHoldersLoading(true)
      try {
        const masterHolders = await client.getJettonWallets(undefined, [formattedAddress])
        if (!isActive) return
        setHolders(masterHolders)
      } catch (error) {
        console.error("Failed to fetch jetton holders", error)
      } finally {
        if (isActive) setHoldersLoading(false)
      }
    }

    void loadHolders()
    return () => {
      isActive = false
    }
  }, [accountAddressKey, activeTab, client, isJettonMasterAccount])

  const handleSearch = (addr: string, event?: ExplorerNavigationClickEvent) => {
    const finalAddr = addr ? normalizeAddress(addr, addressFormat) : ""
    openPath(finalAddr ? routes.addressPath(finalAddr) : routes.rootPath, event)
  }

  const handleTabChange = (tab: string) => {
    const hash = tab === "contract" ? "contract-storage" : tab
    void navigate(`${location.pathname}#${hash}`, {replace: true})
  }

  const tokenInfo = jettonMaster ?? jettonWalletMaster
  const tokenSymbol = tokenInfo?.jetton_content.symbol
  const tokenName = tokenInfo?.jetton_content.name || "Unknown Jetton"
  const tokenDecimals = tokenInfo?.jetton_content.decimals
  const tokenImageSources = getImageSources(tokenInfo?.jetton_content, TOKEN_IMAGE_SOURCE_KEYS)
  const tokenImage = tokenImageSources[0] ?? TOKEN_PLACEHOLDER_IMAGE
  const jettonMasterAdminAddress = jettonMaster?.admin_address ?? undefined
  const tokenTotalSupply = jettonMaster
    ? formatJettonAmount(jettonMaster.total_supply, tokenDecimals)
    : undefined
  const tokenTotalSupplyLabel = tokenTotalSupply
    ? `${tokenTotalSupply}${tokenSymbol ? ` ${tokenSymbol}` : ""}`
    : undefined
  const jettonWalletAmount =
    jettonWalletAccount && jettonWalletMaster
      ? formatJettonAmount(jettonWalletAccount.balance, jettonWalletMaster.jetton_content.decimals)
      : undefined
  const jettonWalletAmountLabel = jettonWalletAmount
    ? `${jettonWalletAmount}${tokenSymbol ? ` ${tokenSymbol}` : ""}`
    : undefined
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
  const nftItemImage = nftItemImageSources[0] ?? TOKEN_PLACEHOLDER_IMAGE
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
    ...getImageSources(nftCollectionTokenInfo, NFT_IMAGE_SOURCE_KEYS),
    ...getImageSources(collectionSample?.content, NFT_COLLECTION_IMAGE_SOURCE_KEYS),
  ]
  const nftCollectionImage = nftCollectionImageSources[0] ?? TOKEN_PLACEHOLDER_IMAGE
  const collectiblePreviews = nftItems.slice(0, 8).map(item => {
    const imageSources = getImageSources(item.content, NFT_IMAGE_SOURCE_KEYS)
    return {
      image: imageSources[0] ?? TOKEN_PLACEHOLDER_IMAGE,
      imageSources,
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
  const showAccountHeader = accountLoading || Boolean(accountState) || accountUnavailable
  const hasHeaderContextCard = Boolean(
    accountState && (tokenInfo || currentNftItem || (nftCollectionName && !currentNftItem)),
  )
  const topSectionClassName = hasHeaderContextCard
    ? styles.topSection
    : `${styles.topSection} ${styles.topSectionSingle}`

  return (
    <div className={styles.container}>
      {formattedAddress && (
        <>
          <Breadcrumbs
            items={[
              {
                label: formattedAddress,
                isAddress: true,
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
                  amount={jettonWalletAmountLabel}
                  amountLoading={isJettonWalletAccount && jettonWalletLoading}
                  client={client}
                  onMoreAssetsClick={() => handleTabChange("tokens")}
                  collectiblesCount={nftItems.length}
                  collectiblePreviews={collectiblePreviews}
                  collectiblesLoading={accountLoading || nftItemsLoading}
                  onCollectiblesClick={() => handleTabChange("nfts")}
                  hasContextCard={hasHeaderContextCard}
                />
              )}
              {hasHeaderContextCard && (
                <div className={styles.contextColumn}>
                  {accountState && tokenInfo && (
                    <div
                      className={`${styles.jettonInfo} ${jettonMaster ? styles.jettonMasterInfo : ""}`}
                    >
                      <div className={styles.jettonHeader}>
                        <img
                          src={tokenImage}
                          alt={tokenName}
                          className={styles.jettonImage}
                          onError={event =>
                            replaceBrokenImageWithFallback(event, tokenImageSources)
                          }
                        />
                        <div className={styles.jettonHeaderContent}>
                          <div className={styles.jettonTitle}>
                            <div className={styles.jettonName}>{tokenName}</div>
                            {tokenSymbol && (
                              <div className={styles.jettonSymbol}>{tokenSymbol}</div>
                            )}
                          </div>
                          {jettonMaster && tokenTotalSupplyLabel && (
                            <div className={styles.jettonSupply}>
                              Max.supply: {tokenTotalSupplyLabel}
                            </div>
                          )}
                          {jettonMaster && (
                            <button
                              type="button"
                              className={styles.jettonMetadataButton}
                              onClick={() => setJettonMetadataOpen(true)}
                            >
                              Metadata
                            </button>
                          )}
                        </div>
                      </div>
                      {!jettonMaster && jettonWalletAccount && jettonWalletMaster && (
                        <>
                          <div className={styles.jettonDivider} />
                          <AccountDetailRows>
                            <AccountAddressDetailRow
                              label="Jetton master"
                              address={jettonWalletAccount.jetton}
                              onAddressClick={handleSearch}
                            />
                            <AccountAddressDetailRow
                              label="Holder address"
                              address={jettonWalletAccount.owner}
                              onAddressClick={handleSearch}
                            />
                          </AccountDetailRows>
                        </>
                      )}
                    </div>
                  )}
                  {accountState && currentNftItem && (
                    <div className={styles.nftPanel}>
                      <div className={styles.nftPanelHeader}>
                        <div className={styles.nftPanelHeading}>
                          <div className={styles.nftPanelTitle}>{nftItemName}</div>
                          <button
                            type="button"
                            className={styles.nftPanelMetadataButton}
                            onClick={() => setJettonMetadataOpen(true)}
                          >
                            Metadata
                          </button>
                        </div>
                      </div>
                      <div className={styles.nftPanelDivider} />
                      <div className={styles.nftPanelBody}>
                        <div className={styles.nftPanelMain}>
                          <AccountDetailRows>
                            <AccountAddressDetailRow
                              label="Owner"
                              address={nftItemOwnerAddress}
                              fallback="No owner"
                              onAddressClick={handleSearch}
                            />
                            <AccountAddressDetailRow
                              label="Collection Address"
                              address={nftItemCollectionAddress}
                              fallback="Standalone"
                              onAddressClick={handleSearch}
                            />
                            <AccountTextDetailRow
                              label="Index"
                              value={`#${currentNftItem.index}`}
                            />
                          </AccountDetailRows>
                          {nftItemDescription && (
                            <div className={styles.nftPanelDescription}>{nftItemDescription}</div>
                          )}
                        </div>
                        <div className={styles.nftPanelMedia}>
                          <img
                            src={nftItemImage}
                            alt={nftItemName}
                            className={styles.nftPanelImage}
                            onError={event =>
                              replaceBrokenImageWithFallback(event, nftItemImageSources)
                            }
                          />
                        </div>
                      </div>
                    </div>
                  )}
                  {accountState && nftCollectionName && !currentNftItem && (
                    <div className={styles.nftPanel}>
                      <div className={styles.nftPanelHeader}>
                        <div className={styles.nftPanelHeading}>
                          <div className={styles.nftPanelTitle}>{nftCollectionName}</div>
                        </div>
                      </div>
                      <div className={styles.nftPanelDivider} />
                      <div className={styles.nftPanelBody}>
                        <div className={styles.nftPanelMain}>
                          <AccountDetailRows>
                            <AccountTextDetailRow
                              label="Indexed items"
                              value={currentNftCollectionItems.length.toLocaleString()}
                            />
                            {collectionSample && (
                              <AccountAddressDetailRow
                                label="Latest item"
                                address={collectionSample.address}
                                onAddressClick={handleSearch}
                              />
                            )}
                          </AccountDetailRows>
                          {nftCollectionDescription && (
                            <div className={styles.nftPanelDescription}>
                              {nftCollectionDescription}
                            </div>
                          )}
                        </div>
                        <div className={styles.nftPanelMedia}>
                          <img
                            src={nftCollectionImage}
                            alt={nftCollectionName}
                            className={styles.nftPanelImage}
                            onError={event =>
                              replaceBrokenImageWithFallback(event, nftCollectionImageSources)
                            }
                          />
                        </div>
                      </div>
                    </div>
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
            jettonMaster={jettonMaster}
            holders={holders}
            tokensLoading={jettonWalletsLoading}
            nftsLoading={nftItemsLoading}
            holdersLoading={holdersLoading}
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
            accountLoading={accountLoading}
            showHoldersTab={isJettonMasterAccount}
            client={client}
            onAddressClick={handleSearch}
            onTransactionClick={handleTransactionClick}
            onLoadMoreTransactions={loadMoreTransactions}
            onLoadMoreActions={loadMoreActions}
            activeTabHash={activeTab}
            onTabChange={handleTabChange}
          />
          {jettonMetadataOpen && activeMetadataJson && (
            <div
              className={styles.metadataOverlay}
              role="presentation"
              onClick={event => {
                if (event.target === event.currentTarget) {
                  setJettonMetadataOpen(false)
                }
              }}
            >
              <section
                className={styles.metadataDialog}
                role="dialog"
                aria-modal="true"
                aria-labelledby="account-metadata-title"
              >
                <button
                  type="button"
                  className={styles.metadataCloseButton}
                  onClick={() => setJettonMetadataOpen(false)}
                  aria-label="Close metadata"
                >
                  <X size={18} strokeWidth={3} />
                </button>
                <h2 id="account-metadata-title" className={styles.metadataTitle}>
                  Metadata
                </h2>
                <div className={styles.metadataHero}>
                  <div className={styles.metadataMain}>
                    <div className={styles.metadataTokenTitle}>
                      <span>{activeMetadataTitle}</span>
                    </div>
                    <div className={styles.metadataSummary}>
                      <div className={styles.metadataRow}>
                        <span className={styles.metadataLabel}>Address</span>
                        <span className={`${styles.metadataValue} ${styles.metadataLink}`}>
                          <AddressLabel address={formattedAddress} />
                        </span>
                      </div>
                      {jettonMaster && (
                        <>
                          <div className={styles.metadataRow}>
                            <span className={styles.metadataLabel}>Owner</span>
                            {jettonMasterAdminAddress ? (
                              <span
                                className={`${styles.metadataValue} ${styles.metadataLink}`}
                                onClick={() => handleSearch(jettonMasterAdminAddress)}
                                onKeyDown={event => {
                                  if (event.key === "Enter" || event.key === " ") {
                                    handleSearch(jettonMasterAdminAddress)
                                  }
                                }}
                                role="button"
                                tabIndex={0}
                              >
                                <AddressLabel address={jettonMasterAdminAddress} />
                              </span>
                            ) : (
                              <span className={styles.metadataValue}>None</span>
                            )}
                          </div>
                          {tokenTotalSupplyLabel && (
                            <div className={styles.metadataRow}>
                              <span className={styles.metadataLabel}>Max.supply</span>
                              <span className={styles.metadataValue}>{tokenTotalSupplyLabel}</span>
                            </div>
                          )}
                          <div className={styles.metadataRow}>
                            <span className={styles.metadataLabel}>Mintable</span>
                            <span className={styles.metadataValue}>
                              {String(jettonMaster.mintable)}
                            </span>
                          </div>
                        </>
                      )}
                      {currentNftItem && (
                        <>
                          <div className={styles.metadataRow}>
                            <span className={styles.metadataLabel}>Owner</span>
                            {nftItemOwnerAddress ? (
                              <span
                                className={`${styles.metadataValue} ${styles.metadataLink}`}
                                onClick={() => handleSearch(nftItemOwnerAddress)}
                                onKeyDown={event => {
                                  if (event.key === "Enter" || event.key === " ") {
                                    handleSearch(nftItemOwnerAddress)
                                  }
                                }}
                                role="button"
                                tabIndex={0}
                              >
                                <AddressLabel address={nftItemOwnerAddress} />
                              </span>
                            ) : (
                              <span className={styles.metadataValue}>No owner</span>
                            )}
                          </div>
                          <div className={styles.metadataRow}>
                            <span className={styles.metadataLabel}>Collection</span>
                            {nftItemCollectionAddress ? (
                              <span
                                className={`${styles.metadataValue} ${styles.metadataLink}`}
                                onClick={() => handleSearch(nftItemCollectionAddress)}
                                onKeyDown={event => {
                                  if (event.key === "Enter" || event.key === " ") {
                                    handleSearch(nftItemCollectionAddress)
                                  }
                                }}
                                role="button"
                                tabIndex={0}
                              >
                                <AddressLabel address={nftItemCollectionAddress} />
                              </span>
                            ) : (
                              <span className={styles.metadataValue}>Standalone</span>
                            )}
                          </div>
                          <div className={styles.metadataRow}>
                            <span className={styles.metadataLabel}>Index</span>
                            <span className={styles.metadataValue}>#{currentNftItem.index}</span>
                          </div>
                        </>
                      )}
                    </div>
                  </div>
                  {activeMetadataImage && (
                    <img
                      src={activeMetadataImage}
                      alt={activeMetadataTitle}
                      className={`${styles.metadataTokenImage} ${
                        currentNftItem ? styles.metadataNftImage : ""
                      }`}
                      onError={event =>
                        replaceBrokenImageWithFallback(event, activeMetadataImageSources)
                      }
                    />
                  )}
                </div>
                {(jettonMaster?.jetton_content.description || nftItemDescription) && (
                  <p className={styles.metadataDescription}>
                    {jettonMaster?.jetton_content.description || nftItemDescription}
                  </p>
                )}
                <div className={styles.metadataJsonFrame}>
                  <button
                    type="button"
                    className={styles.metadataJsonCopyButton}
                    onClick={() => {
                      void navigator.clipboard.writeText(activeMetadataJson)
                      setJettonMetadataCopied(true)
                    }}
                    aria-label={jettonMetadataCopied ? "Metadata copied" : "Copy metadata JSON"}
                    title={jettonMetadataCopied ? "Copied" : "Copy metadata JSON"}
                  >
                    <Copy size={18} />
                  </button>
                  <pre className={styles.metadataJson}>
                    <code>{renderJson(activeMetadataJson)}</code>
                  </pre>
                </div>
              </section>
            </div>
          )}
        </>
      )}

      {!accountState && !accountLoading && !accountError && formattedAddress && (
        <div className={styles.empty}>No data found for this address.</div>
      )}
    </div>
  )
}

function formatJettonAmount(value: string, decimals?: string): string {
  const decimalsNumber = Number(decimals || 9)
  return (Number(value) / 10 ** decimalsNumber).toLocaleString(undefined, {
    maximumFractionDigits: decimalsNumber,
  })
}

function sortJettonWalletsByAmount(wallets: readonly JettonWallet[]): JettonWallet[] {
  return [...wallets].sort(compareJettonWalletAmount)
}

function compareJettonWalletAmount(left: JettonWallet, right: JettonWallet): number {
  const leftBalance = parseBigIntAmount(left.balance)
  const rightBalance = parseBigIntAmount(right.balance)
  const leftDecimals = parseJettonDecimals(left.master?.jetton_content.decimals)
  const rightDecimals = parseJettonDecimals(right.master?.jetton_content.decimals)
  const leftScaled = leftBalance * 10n ** BigInt(rightDecimals)
  const rightScaled = rightBalance * 10n ** BigInt(leftDecimals)

  if (leftScaled > rightScaled) return -1
  if (leftScaled < rightScaled) return 1

  const leftSymbol = left.master?.jetton_content.symbol ?? ""
  const rightSymbol = right.master?.jetton_content.symbol ?? ""
  return leftSymbol.localeCompare(rightSymbol)
}

function parseBigIntAmount(value: string): bigint {
  try {
    return BigInt(value)
  } catch {
    return 0n
  }
}

function parseJettonDecimals(decimals: string | undefined): number {
  const value = Number(decimals ?? 9)
  if (!Number.isInteger(value) || value < 0) {
    return 9
  }
  return Math.min(value, 36)
}

interface CopyAddressButtonProps {
  readonly address: string
  readonly className?: string
  readonly title?: string
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

const CopyAddressButton: FC<CopyAddressButtonProps> = ({
  address,
  className,
  title = "Copy address",
}) => {
  const [isCopied, setIsCopied] = useState(false)

  useEffect(() => {
    if (!isCopied) {
      return
    }

    const timer = setTimeout(() => setIsCopied(false), 1600)
    return () => clearTimeout(timer)
  }, [isCopied])

  return (
    <button
      type="button"
      className={`${styles.addressCopyButton} ${isCopied ? styles.addressCopyButtonCopied : ""} ${
        className ?? ""
      }`}
      onClick={event => {
        event.stopPropagation()
        void navigator.clipboard.writeText(address)
        setIsCopied(true)
      }}
      aria-label={isCopied ? "Address copied" : title}
      title={isCopied ? "Copied" : title}
    >
      {isCopied ? <Check size={14} /> : <Copy size={14} />}
    </button>
  )
}

interface AccountDetailRowsProps {
  readonly children: ReactNode
}

const AccountDetailRows: FC<AccountDetailRowsProps> = ({children}) => (
  <div className={styles.accountDetailRows}>{children}</div>
)

interface AccountTextDetailRowProps {
  readonly label: string
  readonly value: ReactNode
}

const AccountTextDetailRow: FC<AccountTextDetailRowProps> = ({label, value}) => (
  <div className={styles.accountDetailRow}>
    <span className={styles.accountDetailLabel}>{label}</span>
    <span className={styles.accountDetailValue}>{value}</span>
  </div>
)

interface AccountAddressDetailRowProps {
  readonly label: string
  readonly address?: string
  readonly fallback?: string
  readonly onAddressClick: (address: string) => void
}

const AccountAddressDetailRow: FC<AccountAddressDetailRowProps> = ({
  label,
  address,
  fallback,
  onAddressClick,
}) => (
  <div className={styles.accountDetailRow}>
    <span className={styles.accountDetailLabel}>{label}</span>
    {address ? (
      <div className={styles.accountDetailAddressValue}>
        <span
          className={`${styles.accountDetailValue} ${styles.accountDetailLink}`}
          onClick={() => onAddressClick(address)}
          onKeyDown={event => {
            if (event.key === "Enter" || event.key === " ") {
              onAddressClick(address)
            }
          }}
          role="button"
          tabIndex={0}
        >
          <AddressLabel address={address} />
        </span>
        <CopyAddressButton address={address} />
      </div>
    ) : (
      <span className={styles.accountDetailValue}>{fallback ?? "Unknown"}</span>
    )}
  </div>
)

const JSON_TOKEN_RE =
  /("(?:\\.|[^"\\])*")(\s*:)?|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null)/g

function renderJson(json: string): ReactNode[] {
  const parts: ReactNode[] = []
  let lastIndex = 0
  let key = 0

  for (const match of json.matchAll(JSON_TOKEN_RE)) {
    if (match.index === undefined) continue

    if (match.index > lastIndex) {
      parts.push(json.slice(lastIndex, match.index))
    }

    const [token, stringToken, colon, literalToken] = match
    if (stringToken) {
      parts.push(
        <span
          key={`json-token-${key++}`}
          className={colon ? styles.metadataJsonKey : styles.metadataJsonValue}
        >
          {stringToken}
        </span>,
      )
      if (colon) {
        parts.push(colon)
      }
    } else if (literalToken) {
      parts.push(
        <span key={`json-token-${key++}`} className={styles.metadataJsonValue}>
          {literalToken}
        </span>,
      )
    } else {
      parts.push(token)
    }

    lastIndex = match.index + token.length
  }

  if (lastIndex < json.length) {
    parts.push(json.slice(lastIndex))
  }

  return parts
}

function getAccountTokenInfo(
  stateV3: AccountStatesResponse | void,
): readonly AccountStateTokenInfo[] {
  if (!stateV3) return []
  const currentAccount = stateV3.accounts[0]
  return currentAccount ? (stateV3.metadata[currentAccount.address]?.token_info ?? []) : []
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
    value === "tokens" ||
    value === "nfts" ||
    value === "holders"
  )
}

function hasAccountInterface(interfaces: readonly string[], expected: string): boolean {
  return interfaces.some(iface => iface.trim().toLowerCase() === expected)
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

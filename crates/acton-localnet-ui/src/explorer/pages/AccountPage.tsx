import type React from "react"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {useEffect, useMemo, useState} from "react"
import {useLocation, useNavigate, useParams} from "react-router-dom"

import type {TonClient} from "../api/client"
import type {
  AccountStatesResponse,
  AccountStateTokenInfo,
  FullAccountState,
  JettonMaster,
  JettonWallet,
  NftItem,
  Transaction,
  V3AccountState,
} from "../api/types"
import {AccountInfo} from "../components/AccountInfo"
import {AddressLabel} from "../components/AddressLabel"
import {Breadcrumbs} from "../components/Breadcrumbs"
import {AccountDetails} from "../components/AccountDetails"
import {normalizeAddress} from "../components/utils"
import {useAddressFormat} from "../hooks/useNetworkInfo"

import styles from "./AccountPage.module.css"

interface AccountPageProps {
  readonly client: TonClient
}

const NFT_PLACEHOLDER_IMAGE = "/token-placeholder.svg"
const ACCOUNT_TRANSACTION_HISTORY_LIMIT = 1000
type AccountTab = "history" | "contract" | "tokens" | "nfts" | "holders"

export const AccountPage: React.FC<AccountPageProps> = ({client}) => {
  const {address = ""} = useParams<{address: string}>()
  const navigate = useNavigate()
  const location = useLocation()
  const addressFormat = useAddressFormat()
  const [accountState, setAccountState] = useState<FullAccountState | undefined>()
  const [accountStateV3, setAccountStateV3] = useState<V3AccountState | undefined>()
  const [transactions, setTransactions] = useState<Transaction[]>([])
  const [jettonMaster, setJettonMaster] = useState<JettonMaster | undefined>()
  const [jettonWalletAccount, setJettonWalletAccount] = useState<JettonWallet | undefined>()
  const [jettonWalletMaster, setJettonWalletMaster] = useState<JettonMaster | undefined>()
  const [jettonWallets, setJettonWallets] = useState<JettonWallet[]>([])
  const [accountTokenInfo, setAccountTokenInfo] = useState<readonly AccountStateTokenInfo[]>([])
  const [currentNftItem, setCurrentNftItem] = useState<NftItem | undefined>()
  const [currentNftCollectionItems, setCurrentNftCollectionItems] = useState<NftItem[]>([])
  const [nftItems, setNftItems] = useState<NftItem[]>([])
  const [holders, setHolders] = useState<JettonWallet[]>([])
  const [jettonWalletsLoading, setJettonWalletsLoading] = useState(false)
  const [nftItemsLoading, setNftItemsLoading] = useState(false)
  const [holdersLoading, setHoldersLoading] = useState(false)
  const [transactionsLoading, setTransactionsLoading] = useState(true)
  const [transactionsError, setTransactionsError] = useState<string | undefined>()
  const [accountLoading, setAccountLoading] = useState(true)
  const [accountError, setAccountError] = useState<string | undefined>()
  const [compilerAbi, setCompilerAbi] = useState<ContractABI | undefined>()
  const [compilerAbiLoading, setCompilerAbiLoading] = useState(false)
  const [compilerAbiError, setCompilerAbiError] = useState<string | undefined>()

  const formattedAddress = useMemo(
    () => normalizeAddress(address, addressFormat),
    [address, addressFormat],
  )
  const activeTab = useMemo<AccountTab>(() => {
    const tab = location.hash.replace("#", "")
    return isAccountTab(tab) ? tab : "history"
  }, [location.hash])
  const accountInterfaces = accountStateV3?.interfaces ?? []
  const accountCodeHash = accountStateV3?.code_hash
  const isJettonMasterAccount = hasAccountInterface(accountInterfaces, "jetton_master")
  const isJettonWalletAccount = hasAccountInterface(accountInterfaces, "jetton_wallet")
  const isNftItemAccount = hasAccountInterface(accountInterfaces, "nft_item")
  const isNftCollectionAccount = hasAccountInterface(accountInterfaces, "nft_collection")

  useEffect(() => {
    let isActive = true
    const load = () => {
      if (!formattedAddress) {
        setAccountState(undefined)
        setAccountStateV3(undefined)
        setTransactions([])
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
        setNftItemsLoading(false)
        setHoldersLoading(false)
        setTransactionsLoading(false)
        setTransactionsError(undefined)
        setAccountLoading(false)
        setAccountError(undefined)
        return
      }
      setAccountLoading(true)
      setAccountError(undefined)
      setTransactionsLoading(true)
      setTransactionsError(undefined)
      setAccountState(undefined)
      setAccountStateV3(undefined)
      setTransactions([])
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
      setNftItemsLoading(false)
      setHoldersLoading(false)

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
          setNftItemsLoading(false)
          setHoldersLoading(false)
          setTransactionsLoading(false)
        } finally {
          if (isActive) setAccountLoading(false)
        }
      }

      const loadTransactions = async () => {
        try {
          const txs = await client.getTransactions(
            formattedAddress,
            ACCOUNT_TRANSACTION_HISTORY_LIMIT,
          )
          if (!isActive) return
          setTransactions(txs)
          setTransactionsError(undefined)
        } catch (error) {
          if (!isActive) return
          console.error("Failed to fetch account transactions", error)
          setTransactions([])
          setTransactionsError(
            error instanceof Error ? error.message : "Failed to load transactions",
          )
        } finally {
          if (isActive) setTransactionsLoading(false)
        }
      }

      void loadAccountState()
      void loadTransactions()
    }

    load()
    return () => {
      isActive = false
    }
  }, [client, formattedAddress])

  useEffect(() => {
    let isActive = true

    const loadCompilerAbi = async () => {
      if (!accountCodeHash) {
        setCompilerAbi(undefined)
        setCompilerAbiLoading(false)
        setCompilerAbiError(undefined)
        return
      }

      setCompilerAbi(undefined)
      setCompilerAbiLoading(true)
      setCompilerAbiError(undefined)

      try {
        const abis = await client.getCompilerAbis([accountCodeHash])
        if (!isActive) return
        setCompilerAbi(abis[accountCodeHash] ?? undefined)
        setCompilerAbiLoading(false)
      } catch (error) {
        if (!isActive) return
        setCompilerAbi(undefined)
        setCompilerAbiLoading(false)
        setCompilerAbiError(error instanceof Error ? error.message : "Failed to load compiler ABI")
      }
    }

    void loadCompilerAbi()
    return () => {
      isActive = false
    }
  }, [accountCodeHash, client])

  useEffect(() => {
    if (!formattedAddress) {
      return
    }

    let isActive = true
    let refreshInFlight = false
    let refreshQueued = false
    const seenTransactionHashes = new Set<string>()

    const refreshAccount = async () => {
      if (refreshInFlight) {
        refreshQueued = true
        return
      }

      refreshInFlight = true
      try {
        do {
          refreshQueued = false
          const [nextState, nextStateV3, nextTransactions] = await Promise.all([
            client.getAddressInformation(formattedAddress),
            client.getAccountStates([formattedAddress], false).catch(() => {}),
            client.getTransactions(formattedAddress, ACCOUNT_TRANSACTION_HISTORY_LIMIT),
          ])
          if (!isActive) return
          setAccountState(nextState)
          setAccountStateV3(nextStateV3 ? nextStateV3.accounts[0] : undefined)
          setAccountTokenInfo(getAccountTokenInfo(nextStateV3))
          setTransactions(nextTransactions)
          setTransactionsError(undefined)
          setTransactionsLoading(false)
        } while (refreshQueued && isActive)
      } catch (error) {
        if (isActive) {
          console.error("Failed to refresh account data", error)
        }
      } finally {
        refreshInFlight = false
      }
    }

    const unsubscribe = client.subscribeAccountTransactions(formattedAddress, {
      onTransactions: event => {
        if (event.finality === "pending") {
          return
        }

        const hashes = event.transactions.map(tx => tx.hash).filter(Boolean)
        const hasUnseenTransaction = hashes.some(hash => !seenTransactionHashes.has(hash))
        for (const hash of hashes) {
          seenTransactionHashes.add(hash)
        }

        if (hasUnseenTransaction) {
          void refreshAccount()
        }
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
  }, [client, formattedAddress])

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
  }, [client, formattedAddress, isJettonMasterAccount])

  useEffect(() => {
    let isActive = true

    const loadJettonWallet = async () => {
      if (!formattedAddress || !isJettonWalletAccount) {
        setJettonWalletAccount(undefined)
        setJettonWalletMaster(undefined)
        return
      }

      try {
        const currentWallets = await client.getJettonWalletsByAddress([formattedAddress])
        const currentWallet = currentWallets[0]
        const currentWalletMasters = currentWallet
          ? await client.getJettonMasters([currentWallet.jetton])
          : []
        if (!isActive) return
        setJettonWalletAccount(currentWallet)
        setJettonWalletMaster(currentWalletMasters[0])
      } catch (error) {
        console.error("Failed to fetch jetton wallet", error)
      }
    }

    void loadJettonWallet()
    return () => {
      isActive = false
    }
  }, [client, formattedAddress, isJettonWalletAccount])

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
        setJettonWallets(wallets)
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
  }, [client, formattedAddress])

  useEffect(() => {
    let isActive = true

    const loadNftItem = async () => {
      if (!formattedAddress || !isNftItemAccount) {
        setCurrentNftItem(undefined)
        return
      }

      try {
        const items = await client.getNftItems({address: [formattedAddress], limit: 1})
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
  }, [client, formattedAddress, isNftItemAccount])

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
  }, [client, formattedAddress, isNftCollectionAccount])

  useEffect(() => {
    let isActive = true

    const loadNftItems = async () => {
      if (!formattedAddress || activeTab !== "nfts") {
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
  }, [activeTab, client, formattedAddress])

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
  }, [activeTab, client, formattedAddress, isJettonMasterAccount])

  const handleSearch = (addr: string) => {
    const finalAddr = addr ? normalizeAddress(addr, addressFormat) : ""
    if (finalAddr) {
      void navigate(`/explorer/address/${finalAddr}`)
    } else {
      void navigate("/explorer")
    }
  }

  const handleTabChange = (tab: string) => {
    void navigate(`${location.pathname}#${tab}`, {replace: true})
  }

  const tokenInfo = jettonMaster ?? jettonWalletMaster
  const tokenSymbol = tokenInfo?.jetton_content.symbol
  const nftItemTokenInfo = accountTokenInfo.find(info => info.type === "nft_items")
  const nftCollectionTokenInfo = accountTokenInfo.find(info => info.type === "nft_collections")
  const nftItemName =
    tokenInfoString(nftItemTokenInfo, "name") ||
    contentString(currentNftItem?.content, "name") ||
    (currentNftItem ? `NFT #${currentNftItem.index}` : undefined)
  const nftItemDescription =
    tokenInfoString(nftItemTokenInfo, "description") ||
    contentString(currentNftItem?.content, "description")
  const nftItemImage =
    tokenInfoString(nftItemTokenInfo, "image") ||
    contentString(currentNftItem?.content, "image") ||
    contentString(currentNftItem?.content, "preview") ||
    contentString(currentNftItem?.content, "image_url") ||
    NFT_PLACEHOLDER_IMAGE
  const collectionSample = currentNftCollectionItems[0]
  const nftCollectionName =
    tokenInfoString(nftCollectionTokenInfo, "name") ||
    contentString(collectionSample?.content, "collection_name") ||
    (nftCollectionTokenInfo || currentNftCollectionItems.length > 0 ? "NFT Collection" : undefined)
  const nftCollectionDescription =
    tokenInfoString(nftCollectionTokenInfo, "description") ||
    contentString(collectionSample?.content, "collection_description")
  const nftCollectionImage =
    tokenInfoString(nftCollectionTokenInfo, "image") ||
    contentString(collectionSample?.content, "collection_image") ||
    NFT_PLACEHOLDER_IMAGE

  return (
    <div className={styles.container}>
      {accountError && <div className={styles.error}>{accountError}</div>}

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
          <div className={styles.topSection}>
            <AccountInfo
              address={formattedAddress}
              state={accountState}
              compilerAbi={compilerAbi}
              contractInterfaces={accountStateV3?.interfaces}
              jettonWallets={jettonWallets}
              accountLoading={accountLoading}
              assetsLoading={accountLoading || jettonWalletsLoading}
              client={client}
              onMoreAssetsClick={() => handleTabChange("tokens")}
            />
            {accountState && tokenInfo && (
              <div className={styles.jettonInfo}>
                <div className={styles.jettonHeader}>
                  {tokenInfo.jetton_content.image && (
                    <img
                      src={tokenInfo.jetton_content.image}
                      alt={tokenInfo.jetton_content.name}
                      className={styles.jettonImage}
                    />
                  )}
                  <div className={styles.jettonTitle}>
                    <div className={styles.jettonName}>
                      {tokenInfo.jetton_content.name || "Unknown Jetton"}
                    </div>
                    <div className={styles.jettonSymbol}>
                      {tokenSymbol && `$${tokenSymbol}`}{" "}
                      {jettonMaster ? "Jetton master" : "Jetton wallet"}
                    </div>
                  </div>
                </div>
                {tokenInfo.jetton_content.description && (
                  <div className={styles.jettonDescription}>
                    {tokenInfo.jetton_content.description}
                  </div>
                )}
                <div className={styles.jettonDetails}>
                  {jettonMaster ? (
                    <>
                      <div className={styles.jettonRow}>
                        <span className={styles.jettonLabel}>Total supply</span>
                        <span className={styles.jettonValue}>
                          {formatJettonAmount(
                            jettonMaster.total_supply,
                            jettonMaster.jetton_content.decimals,
                          )}
                        </span>
                      </div>
                      <div className={styles.jettonRow}>
                        <span className={styles.jettonLabel}>Mintable</span>
                        <span className={styles.jettonValue}>
                          {jettonMaster.mintable ? "Yes" : "No"}
                        </span>
                      </div>
                      <div className={styles.jettonRow}>
                        <span className={styles.jettonLabel}>Admin</span>
                        <span
                          className={`${styles.jettonValue} ${styles.jettonLink}`}
                          onClick={() => handleSearch(jettonMaster.admin_address)}
                          onKeyDown={e => {
                            if (e.key === "Enter" || e.key === " ") {
                              handleSearch(jettonMaster.admin_address)
                            }
                          }}
                          role="button"
                          tabIndex={0}
                        >
                          <AddressLabel address={jettonMaster.admin_address} />
                        </span>
                      </div>
                    </>
                  ) : (
                    jettonWalletAccount &&
                    jettonWalletMaster && (
                      <>
                        <div className={styles.jettonRow}>
                          <span className={styles.jettonLabel}>Wallet balance</span>
                          <span className={styles.jettonValue}>
                            {formatJettonAmount(
                              jettonWalletAccount.balance,
                              jettonWalletMaster.jetton_content.decimals,
                            )}{" "}
                            {tokenSymbol}
                          </span>
                        </div>
                        <div className={styles.jettonRow}>
                          <span className={styles.jettonLabel}>Owner</span>
                          <span
                            className={`${styles.jettonValue} ${styles.jettonLink}`}
                            onClick={() => handleSearch(jettonWalletAccount.owner)}
                            onKeyDown={e => {
                              if (e.key === "Enter" || e.key === " ") {
                                handleSearch(jettonWalletAccount.owner)
                              }
                            }}
                            role="button"
                            tabIndex={0}
                          >
                            <AddressLabel address={jettonWalletAccount.owner} />
                          </span>
                        </div>
                        <div className={styles.jettonRow}>
                          <span className={styles.jettonLabel}>Minter</span>
                          <span
                            className={`${styles.jettonValue} ${styles.jettonLink}`}
                            onClick={() => handleSearch(jettonWalletAccount.jetton)}
                            onKeyDown={e => {
                              if (e.key === "Enter" || e.key === " ") {
                                handleSearch(jettonWalletAccount.jetton)
                              }
                            }}
                            role="button"
                            tabIndex={0}
                          >
                            <AddressLabel address={jettonWalletAccount.jetton} />
                          </span>
                        </div>
                      </>
                    )
                  )}
                </div>
              </div>
            )}
            {accountState && currentNftItem && (
              <div className={styles.jettonInfo}>
                <div className={styles.jettonHeader}>
                  <img src={nftItemImage} alt={nftItemName} className={styles.jettonImage} />
                  <div className={styles.jettonTitle}>
                    <div className={styles.jettonName}>{nftItemName}</div>
                    <div className={styles.jettonSymbol}>NFT item</div>
                  </div>
                </div>
                {nftItemDescription && (
                  <div className={styles.jettonDescription}>{nftItemDescription}</div>
                )}
                <div className={styles.jettonDetails}>
                  <div className={styles.jettonRow}>
                    <span className={styles.jettonLabel}>Index</span>
                    <span className={styles.jettonValue}>#{currentNftItem.index}</span>
                  </div>
                  <div className={styles.jettonRow}>
                    <span className={styles.jettonLabel}>Owner</span>
                    <span
                      className={`${styles.jettonValue} ${styles.jettonLink}`}
                      onClick={() => {
                        if (currentNftItem.owner_address) handleSearch(currentNftItem.owner_address)
                      }}
                      onKeyDown={e => {
                        if ((e.key === "Enter" || e.key === " ") && currentNftItem.owner_address) {
                          handleSearch(currentNftItem.owner_address)
                        }
                      }}
                      role="button"
                      tabIndex={0}
                    >
                      {currentNftItem.owner_address ? (
                        <AddressLabel address={currentNftItem.owner_address} />
                      ) : (
                        "No owner"
                      )}
                    </span>
                  </div>
                  <div className={styles.jettonRow}>
                    <span className={styles.jettonLabel}>Collection</span>
                    <span
                      className={`${styles.jettonValue} ${styles.jettonLink}`}
                      onClick={() => {
                        if (currentNftItem.collection_address) {
                          handleSearch(currentNftItem.collection_address)
                        }
                      }}
                      onKeyDown={e => {
                        if (
                          (e.key === "Enter" || e.key === " ") &&
                          currentNftItem.collection_address
                        ) {
                          handleSearch(currentNftItem.collection_address)
                        }
                      }}
                      role="button"
                      tabIndex={0}
                    >
                      {currentNftItem.collection_address ? (
                        <AddressLabel address={currentNftItem.collection_address} />
                      ) : (
                        "Standalone"
                      )}
                    </span>
                  </div>
                </div>
              </div>
            )}
            {accountState && nftCollectionName && !currentNftItem && (
              <div className={styles.jettonInfo}>
                <div className={styles.jettonHeader}>
                  <img
                    src={nftCollectionImage}
                    alt={nftCollectionName}
                    className={styles.jettonImage}
                  />
                  <div className={styles.jettonTitle}>
                    <div className={styles.jettonName}>{nftCollectionName}</div>
                    <div className={styles.jettonSymbol}>NFT collection</div>
                  </div>
                </div>
                {nftCollectionDescription && (
                  <div className={styles.jettonDescription}>{nftCollectionDescription}</div>
                )}
                <div className={styles.jettonDetails}>
                  <div className={styles.jettonRow}>
                    <span className={styles.jettonLabel}>Indexed items</span>
                    <span className={styles.jettonValue}>
                      {currentNftCollectionItems.length.toLocaleString()}
                    </span>
                  </div>
                  {collectionSample && (
                    <div className={styles.jettonRow}>
                      <span className={styles.jettonLabel}>Latest item</span>
                      <span
                        className={`${styles.jettonValue} ${styles.jettonLink}`}
                        onClick={() => handleSearch(collectionSample.address)}
                        onKeyDown={e => {
                          if (e.key === "Enter" || e.key === " ") {
                            handleSearch(collectionSample.address)
                          }
                        }}
                        role="button"
                        tabIndex={0}
                      >
                        #{collectionSample.index}
                      </span>
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>
          <AccountDetails
            transactions={transactions}
            accountState={accountState}
            compilerAbi={compilerAbi}
            compilerAbiLoading={compilerAbiLoading}
            compilerAbiError={compilerAbiError}
            ownerAddress={formattedAddress}
            jettonWallets={jettonWallets}
            nftItems={nftItems}
            jettonMaster={jettonMaster}
            holders={holders}
            tokensLoading={jettonWalletsLoading}
            nftsLoading={nftItemsLoading}
            holdersLoading={holdersLoading}
            transactionsLoading={transactionsLoading}
            transactionsError={transactionsError}
            accountLoading={accountLoading}
            showHoldersTab={isJettonMasterAccount}
            client={client}
            onAddressClick={handleSearch}
            activeTabHash={activeTab}
            onTabChange={handleTabChange}
          />
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

import type React from "react"
import {useEffect, useMemo, useState} from "react"
import {useLocation, useNavigate, useParams} from "react-router-dom"

import type {TonClient} from "../api/client"
import type {
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
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | undefined>()

  const formattedAddress = useMemo(
    () => normalizeAddress(address, addressFormat),
    [address, addressFormat],
  )

  useEffect(() => {
    let isActive = true
    const load = async () => {
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
        return
      }
      setLoading(true)
      setError(undefined)
      try {
        const [
          state,
          stateV3,
          txs,
          masters,
          wallets,
          nfts,
          masterHolders,
          currentWallets,
          currentNftItems,
          collectionNftItems,
        ] = await Promise.all([
          client.getAddressInformation(formattedAddress),
          client.getAccountStates([formattedAddress], false).catch(() => {}),
          client.getTransactions(formattedAddress),
          client.getJettonMasters([formattedAddress]),
          client.getJettonWallets([formattedAddress]),
          client.getNftItems({
            owner_address: [formattedAddress],
            limit: 100,
            sortByLastTransactionLt: true,
          }),
          client.getJettonWallets(undefined, [formattedAddress]),
          client.getJettonWalletsByAddress([formattedAddress]),
          client.getNftItems({address: [formattedAddress], limit: 1}),
          client.getNftItems({
            collection_address: [formattedAddress],
            limit: 100,
            sortByLastTransactionLt: true,
          }),
        ])
        const currentWallet = currentWallets[0]
        const currentWalletMasters = currentWallet
          ? await client.getJettonMasters([currentWallet.jetton])
          : []
        const currentWalletMaster = currentWalletMasters[0]
        const currentAccount = stateV3?.accounts[0]
        const currentTokenInfo = currentAccount
          ? (stateV3?.metadata[currentAccount.address]?.token_info ?? [])
          : []
        if (!isActive) return
        setAccountState(state)
        setAccountStateV3(stateV3?.accounts[0])
        setTransactions(txs)
        setJettonMaster(masters[0])
        setJettonWalletAccount(currentWallet)
        setJettonWalletMaster(currentWalletMaster)
        setJettonWallets(wallets)
        setAccountTokenInfo(currentTokenInfo)
        setCurrentNftItem(currentNftItems[0])
        setCurrentNftCollectionItems(collectionNftItems)
        setNftItems(nfts)
        setHolders(masterHolders)
      } catch (error) {
        if (!isActive) return
        setError(error instanceof Error ? error.message : "An error occurred")
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
      } finally {
        if (isActive) setLoading(false)
      }
    }

    void load()
    return () => {
      isActive = false
    }
  }, [client, formattedAddress])

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
      {loading && <div className={styles.loading}>Loading...</div>}

      {error && <div className={styles.error}>{error}</div>}

      {accountState && !loading && (
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
              contractInterfaces={accountStateV3?.interfaces}
              jettonWallets={jettonWallets}
              client={client}
              onMoreAssetsClick={() => handleTabChange("tokens")}
            />
            {tokenInfo && (
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
            {currentNftItem && (
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
            {nftCollectionName && !currentNftItem && (
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
            accountCodeHash={accountStateV3?.code_hash}
            ownerAddress={formattedAddress}
            jettonWallets={jettonWallets}
            nftItems={nftItems}
            jettonMaster={jettonMaster}
            holders={holders}
            client={client}
            onAddressClick={handleSearch}
            activeTabHash={location.hash.replace("#", "")}
            onTabChange={handleTabChange}
          />
        </>
      )}

      {!accountState && !loading && !error && formattedAddress && (
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

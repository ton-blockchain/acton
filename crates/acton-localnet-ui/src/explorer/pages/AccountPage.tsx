import type React from "react"
import {useEffect, useMemo, useState} from "react"
import {useLocation, useNavigate, useParams} from "react-router-dom"

import type {TonClient} from "../api/client"
import type {
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

import styles from "./AccountPage.module.css"

interface AccountPageProps {
  readonly client: TonClient
}

export const AccountPage: React.FC<AccountPageProps> = ({client}) => {
  const {address = ""} = useParams<{address: string}>()
  const navigate = useNavigate()
  const location = useLocation()
  const [accountState, setAccountState] = useState<FullAccountState | undefined>()
  const [accountStateV3, setAccountStateV3] = useState<V3AccountState | undefined>()
  const [transactions, setTransactions] = useState<Transaction[]>([])
  const [jettonMaster, setJettonMaster] = useState<JettonMaster | undefined>()
  const [jettonWallets, setJettonWallets] = useState<JettonWallet[]>([])
  const [nftItems, setNftItems] = useState<NftItem[]>([])
  const [holders, setHolders] = useState<JettonWallet[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | undefined>()

  const formattedAddress = useMemo(() => normalizeAddress(address), [address])

  useEffect(() => {
    let isActive = true
    const load = async () => {
      if (!formattedAddress) {
        setAccountState(undefined)
        setAccountStateV3(undefined)
        setTransactions([])
        setJettonMaster(undefined)
        setJettonWallets([])
        setNftItems([])
        setHolders([])
        return
      }
      setLoading(true)
      setError(undefined)
      try {
        const [state, stateV3, txs, masters, wallets, nfts, masterHolders] = await Promise.all([
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
        ])
        if (!isActive) return
        setAccountState(state)
        setAccountStateV3(stateV3?.accounts[0])
        setTransactions(txs)
        setJettonMaster(masters[0])
        setJettonWallets(wallets)
        setNftItems(nfts)
        setHolders(masterHolders)
      } catch (error) {
        if (!isActive) return
        setError(error instanceof Error ? error.message : "An error occurred")
        setAccountState(undefined)
        setAccountStateV3(undefined)
        setTransactions([])
        setJettonMaster(undefined)
        setJettonWallets([])
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
    const finalAddr = addr ? normalizeAddress(addr) : ""
    if (finalAddr) {
      void navigate(`/explorer/address/${finalAddr}`)
    } else {
      void navigate("/explorer")
    }
  }

  const handleTabChange = (tab: string) => {
    void navigate(`${location.pathname}#${tab}`, {replace: true})
  }

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
            {jettonMaster && (
              <div className={styles.jettonInfo}>
                <div className={styles.jettonHeader}>
                  {jettonMaster.jetton_content.image && (
                    <img
                      src={jettonMaster.jetton_content.image}
                      alt={jettonMaster.jetton_content.name}
                      className={styles.jettonImage}
                    />
                  )}
                  <div className={styles.jettonTitle}>
                    <div className={styles.jettonName}>
                      {jettonMaster.jetton_content.name || "Unknown Jetton"}
                    </div>
                    <div className={styles.jettonSymbol}>
                      {jettonMaster.jetton_content.symbol &&
                        `$${jettonMaster.jetton_content.symbol}`}{" "}
                      Jetton master
                    </div>
                  </div>
                </div>
                {jettonMaster.jetton_content.description && (
                  <div className={styles.jettonDescription}>
                    {jettonMaster.jetton_content.description}
                  </div>
                )}
                <div className={styles.jettonDetails}>
                  <div className={styles.jettonRow}>
                    <span className={styles.jettonLabel}>Total supply</span>
                    <span className={styles.jettonValue}>
                      {(
                        Number(jettonMaster.total_supply) /
                        10 ** Number(jettonMaster.jetton_content.decimals || 9)
                      ).toLocaleString()}
                    </span>
                  </div>
                  <div className={styles.jettonRow}>
                    <span className={styles.jettonLabel}>Admin</span>
                    <span
                      className={`${styles.jettonValue} ${styles.jettonLink}`}
                      onClick={() => {
                        if (jettonMaster) handleSearch(jettonMaster.admin_address)
                      }}
                      onKeyDown={e => {
                        if ((e.key === "Enter" || e.key === " ") && jettonMaster) {
                          handleSearch(jettonMaster.admin_address)
                        }
                      }}
                      role="button"
                      tabIndex={0}
                    >
                      <AddressLabel address={jettonMaster.admin_address} />
                    </span>
                  </div>
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

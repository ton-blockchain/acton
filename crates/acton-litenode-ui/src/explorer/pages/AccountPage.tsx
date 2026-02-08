import type React from "react"
import {useEffect, useMemo, useState} from "react"
import {useNavigate, useParams} from "react-router-dom"

import type {TonClient} from "../api/client"
import type {FullAccountState, Transaction} from "../api/types"
import {AccountInfo} from "../components/AccountInfo"
import {Breadcrumbs} from "../components/Breadcrumbs"
import {TransactionList} from "../components/TransactionList"
import {normalizeAddress} from "../components/utils"

import styles from "./AccountPage.module.css"

interface AccountPageProps {
  readonly client: TonClient
}

export const AccountPage: React.FC<AccountPageProps> = ({client}) => {
  const {address = ""} = useParams<{address: string}>()
  const navigate = useNavigate()
  const [accountState, setAccountState] = useState<FullAccountState | undefined>()
  const [transactions, setTransactions] = useState<Transaction[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | undefined>()

  const formattedAddress = useMemo(() => normalizeAddress(address), [address])

  useEffect(() => {
    let isActive = true
    const load = async () => {
      if (!formattedAddress) {
        setAccountState(undefined)
        setTransactions([])
        return
      }
      setLoading(true)
      setError(undefined)
      try {
        const [state, txs] = await Promise.all([
          client.getAddressInformation(formattedAddress),
          client.getTransactions(formattedAddress),
        ])
        if (!isActive) return
        setAccountState(state)
        setTransactions(txs)
      } catch (error) {
        if (!isActive) return
        setError(error instanceof Error ? error.message : "An error occurred")
        setAccountState(undefined)
        setTransactions([])
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
          <AccountInfo address={formattedAddress} state={accountState} />
          <TransactionList
            transactions={transactions}
            accountState={accountState}
            ownerAddress={formattedAddress}
            onAddressClick={handleSearch}
          />
        </>
      )}

      {!accountState && !loading && !error && formattedAddress && (
        <div className={styles.empty}>No data found for this address.</div>
      )}
    </div>
  )
}

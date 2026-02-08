import type React from "react"
import { useEffect, useMemo, useState } from "react"
import type { TonClient } from "../api/client"
import { AccountInfo } from "../components/AccountInfo"
import { Breadcrumbs } from "../components/Breadcrumbs"
import { TransactionList } from "../components/TransactionList"
import styles from "./AccountPage.module.css"
import type { FullAccountState, Transaction } from "../types"
import { normalizeAddress } from "../components/utils"

interface AccountPageProps {
  readonly client: TonClient
  readonly externalAddress?: string
  readonly onAddressChange?: (addr: string) => void
}

export const AccountPage: React.FC<AccountPageProps> = ({
  client,
  externalAddress = "",
  onAddressChange,
}) => {
  const [accountState, setAccountState] = useState<FullAccountState | null>(null)
  const [transactions, setTransactions] = useState<Transaction[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const formattedAddress = useMemo(() => normalizeAddress(externalAddress), [externalAddress])

  useEffect(() => {
    let isActive = true
    const load = async () => {
      if (!formattedAddress) {
        setAccountState(null)
        setTransactions([])
        return
      }
      setLoading(true)
      setError(null)
      try {
        const [state, txs] = await Promise.all([
          client.getAddressInformation(formattedAddress),
          client.getTransactions(formattedAddress),
        ])
        if (!isActive) return
        setAccountState(state)
        setTransactions(txs)
      } catch (e) {
        if (!isActive) return
        setError(e instanceof Error ? e.message : "An error occurred")
        setAccountState(null)
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
            onAddressClick={onAddressChange}
          />
        </>
      )}

      {!accountState && !loading && !error && formattedAddress && (
        <div className={styles.empty}>No data found for this address.</div>
      )}
    </div>
  )
}

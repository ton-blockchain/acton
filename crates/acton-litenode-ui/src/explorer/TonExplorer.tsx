import type React from "react"
import { useEffect, useMemo, useState } from "react"
import type { TonClient } from "./api/client"
import { AccountInfo } from "./components/AccountInfo"
import { Breadcrumbs } from "./components/Breadcrumbs"
import { TransactionList } from "./components/TransactionList"
import styles from "./TonExplorer.module.css"
import type { FullAccountState, Transaction } from "./types"
import { normalizeAddress } from "./components/utils"

interface TonExplorerProps {
  client: TonClient
  externalAddress?: string
  onAddressChange?: (addr: string) => void
}

export const TonExplorer: React.FC<TonExplorerProps> = ({
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
      if (!externalAddress) {
        setAccountState(null)
        setTransactions([])
        return
      }
      setLoading(true)
      setError(null)
      try {
        const [state, txs] = await Promise.all([
          client.getAddressInformation(externalAddress),
          client.getTransactions(externalAddress),
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
  }, [externalAddress, client])

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

import React, {useEffect, useState} from "react"

import {JettonWallet, JettonMaster} from "../api/types"
import {TonClient} from "../api/client"

import styles from "./Tokens.module.css"

interface TokensProps {
  readonly wallets: JettonWallet[]
  readonly client: TonClient
  readonly onAddressClick?: (addr: string) => void
}

interface WalletWithMaster extends JettonWallet {
  readonly master?: JettonMaster
}

export const Tokens: React.FC<TokensProps> = ({wallets, client, onAddressClick}) => {
  const [walletsWithMasters, setWalletsWithMasters] = useState<WalletWithMaster[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    const fetchMasters = async () => {
      setLoading(true)
      try {
        const masterAddresses = [...new Set(wallets.map(w => w.jetton))]
        if (masterAddresses.length === 0) {
          setWalletsWithMasters([])
          return
        }

        const masters = await client.getJettonMasters(masterAddresses)
        const mastersMap = new Map(masters.map(m => [m.address, m]))

        setWalletsWithMasters(
          wallets.map(w => ({
            ...w,
            master: mastersMap.get(w.jetton),
          })),
        )
      } catch (error) {
        console.error("Failed to fetch jetton masters", error)
        setWalletsWithMasters(wallets)
      } finally {
        setLoading(false)
      }
    }

    void fetchMasters()
  }, [wallets, client])

  if (loading) {
    return <div className={styles.empty}>Loading tokens...</div>
  }

  if (wallets.length === 0) {
    return <div className={styles.empty}>No tokens found.</div>
  }

  return (
    <div className={styles.container}>
      <div className={styles.list}>
        {walletsWithMasters.map(w => {
          const decimals = Number(w.master?.jetton_content?.decimals || 9)
          const balance = Number(w.balance) / Math.pow(10, decimals)
          const symbol = w.master?.jetton_content?.symbol || "UNKNOWN"

          if (w.master?.jetton_content?.name == undefined) {
            return
          }

          return (
            <div
              key={w.address}
              className={styles.walletItem}
              onClick={() => onAddressClick?.(w.jetton)}
              onKeyDown={e => {
                if (e.key === "Enter" || e.key === " ") {
                  onAddressClick?.(w.jetton)
                }
              }}
              role="button"
              tabIndex={0}
            >
              <img
                src={
                  w.master?.jetton_content?.image ||
                  "https://wallet.ton.org/assets/img/token-placeholder.svg"
                }
                alt={symbol}
                className={styles.jettonImage}
                onError={e => {
                  ;(e.target as HTMLImageElement).src =
                    "https://wallet.ton.org/assets/img/token-placeholder.svg"
                }}
              />
              <div className={styles.jettonInfoMain}>
                <div className={styles.jettonName}>
                  {w.master?.jetton_content?.name || "Unknown Jetton"}
                </div>
                <div className={styles.jettonBalanceRow}>
                  <span className={styles.balanceValue}>
                    {balance.toLocaleString(undefined, {maximumFractionDigits: decimals})}
                  </span>
                  <span className={styles.jettonSymbol}>{symbol}</span>
                </div>
              </div>
              <div className={styles.priceInfo}>
                <div className={styles.priceValue}>$0.00</div>
                <div className={styles.totalValue}>$0.00</div>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}

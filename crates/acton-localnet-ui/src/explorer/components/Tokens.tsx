import type React from "react"
import {useEffect, useState} from "react"

import type {JettonWallet, JettonMaster} from "../api/types"
import type {TonClient} from "../api/client"

import styles from "./Tokens.module.css"

const TOKEN_PLACEHOLDER_IMAGE = "/token-placeholder.svg"

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
          const rawBalance = Number(w.balance)
          const rawSupply = Number(w.master?.total_supply || "0")
          const balance = rawBalance / 10 ** decimals
          const supplyShare = rawSupply > 0 ? rawBalance / rawSupply : undefined
          const supplyShareLabel =
            supplyShare === undefined
              ? "Unknown"
              : supplyShare === 0
                ? "0%"
                : supplyShare < 0.0001
                  ? "<0.01%"
                  : `${(supplyShare * 100).toLocaleString(undefined, {
                      maximumFractionDigits: supplyShare < 0.01 ? 2 : 1,
                    })}%`
          const symbol = w.master?.jetton_content?.symbol || "UNKNOWN"

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
                src={w.master?.jetton_content?.image || TOKEN_PLACEHOLDER_IMAGE}
                alt={symbol}
                className={styles.jettonImage}
                onError={e => {
                  const img = e.currentTarget
                  if (img.getAttribute("src") === TOKEN_PLACEHOLDER_IMAGE) return
                  img.src = TOKEN_PLACEHOLDER_IMAGE
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
              <div className={styles.supplyInfo}>
                <div className={styles.supplyShareValue}>{supplyShareLabel}</div>
                <div className={styles.supplyShareLabel}>of supply</div>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}

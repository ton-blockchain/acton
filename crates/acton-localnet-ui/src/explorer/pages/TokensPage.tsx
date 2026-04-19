import type React from "react"
import {useEffect, useState} from "react"
import {useNavigate} from "react-router-dom"

import type {TonClient} from "../api/client"
import type {JettonMaster} from "../api/types"

import styles from "./TokensPage.module.css"

interface TokensPageProps {
  readonly client: TonClient
}

export const TokensPage: React.FC<TokensPageProps> = ({client}) => {
  const navigate = useNavigate()
  const [tokens, setTokens] = useState<JettonMaster[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()

  useEffect(() => {
    let isActive = true
    const load = async () => {
      setLoading(true)
      setError(undefined)
      try {
        const masters = await client.getJettonMasters(undefined, 100, 0)
        if (!isActive) return
        setTokens(masters)
      } catch (error) {
        if (!isActive) return
        setError(error instanceof Error ? error.message : "An error occurred")
      } finally {
        if (isActive) setLoading(false)
      }
    }

    void load()
    return () => {
      isActive = false
    }
  }, [client])

  const handleTokenClick = (address: string) => {
    void navigate(`/explorer/address/${address}`)
  }

  return (
    <div className={styles.container}>
      <h1 className={styles.title}>Jettons</h1>

      {loading && <div className={styles.loading}>Loading tokens...</div>}
      {error && <div className={styles.error}>{error}</div>}

      {!loading && !error && (
        <div className={styles.grid}>
          {tokens.map(token => {
            const symbol = token.jetton_content.symbol || "???"
            const name = token.jetton_content.name || "Unknown Jetton"
            const image =
              token.jetton_content.image ||
              "https://wallet.ton.org/assets/img/token-placeholder.svg"
            const decimals = Number(token.jetton_content.decimals || 9)
            const totalSupply = (Number(token.total_supply) / 10 ** decimals).toLocaleString()

            return (
              <div
                key={token.address}
                className={styles.card}
                onClick={() => handleTokenClick(token.address)}
                onKeyDown={e => {
                  if (e.key === "Enter" || e.key === " ") {
                    handleTokenClick(token.address)
                  }
                }}
                role="button"
                tabIndex={0}
              >
                <div className={styles.cardHeader}>
                  <img
                    src={image}
                    alt={symbol}
                    className={styles.tokenImage}
                    onError={e => {
                      ;(e.target as HTMLImageElement).src =
                        "https://wallet.ton.org/assets/img/token-placeholder.svg"
                    }}
                  />
                  <div className={styles.tokenInfo}>
                    <div className={styles.tokenName}>{name}</div>
                    <div className={styles.tokenSymbol}>{symbol}</div>
                  </div>
                </div>
                <div className={styles.cardBody}>
                  <div className={styles.detailRow}>
                    <span className={styles.detailLabel}>Total Supply:</span>
                    <span className={styles.detailValue}>{totalSupply}</span>
                  </div>
                  <div className={styles.detailRow}>
                    <span className={styles.detailLabel}>Mintable:</span>
                    <span className={styles.detailValue}>{token.mintable ? "Yes" : "No"}</span>
                  </div>
                </div>
              </div>
            )
          })}
        </div>
      )}

      {!loading && !error && tokens.length === 0 && (
        <div className={styles.empty}>No Jetton Masters found.</div>
      )}
    </div>
  )
}

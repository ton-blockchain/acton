import * as React from "react"
import {Card, CardContent, CardDescription, CardHeader, CardTitle} from "@acton/shared-ui"
import {useNavigate} from "react-router-dom"

import type {TonClient} from "../../explorer/api/client"
import type {JettonMaster} from "../../explorer/api/types"
import {TOKEN_PLACEHOLDER_IMAGE} from "../constants"
import {formatTokenSupply} from "../dashboardUtils"

import styles from "../DashboardPage.module.css"

interface TokensPageProps {
  readonly client: TonClient
}

interface TokensState {
  readonly items: readonly JettonMaster[]
  readonly isLoading: boolean
  readonly error?: string
}

export const TokensPage: React.FC<TokensPageProps> = ({client}) => {
  const navigate = useNavigate()
  const [tokensState, setTokensState] = React.useState<TokensState>({
    items: [],
    isLoading: true,
  })

  React.useEffect(() => {
    let cancelled = false

    void (async () => {
      setTokensState({
        items: [],
        isLoading: true,
      })

      try {
        const tokens = await client.getJettonMasters(undefined, 100, 0)
        if (cancelled) {
          return
        }
        setTokensState({
          items: tokens,
          isLoading: false,
        })
      } catch (error) {
        if (cancelled) {
          return
        }
        setTokensState({
          items: [],
          isLoading: false,
          error: error instanceof Error ? error.message : "Failed to load tokens",
        })
      }
    })()

    return () => {
      cancelled = true
    }
  }, [client])

  return (
    <>
      <section className={styles.hero}>
        <div>
          <h1 className={styles.title}>Tokens</h1>
          <p className={styles.subtitle}>Jettons detected on the local network.</p>
        </div>
      </section>

      <section className={styles.resourceGrid}>
        {tokensState.error ? (
          <div className={styles.emptyState}>{tokensState.error}</div>
        ) : tokensState.isLoading ? (
          <div className={styles.emptyState}>Loading tokens…</div>
        ) : tokensState.items.length === 0 ? (
          <div className={styles.emptyState}>No tokens yet.</div>
        ) : (
          tokensState.items.map(token => {
            const symbol = token.jetton_content.symbol || "???"
            const name = token.jetton_content.name || "Unknown Jetton"
            const image = token.jetton_content.image || TOKEN_PLACEHOLDER_IMAGE
            const href = `/explorer/address/${encodeURIComponent(token.address)}`

            return (
              <Card
                key={token.address}
                className={`${styles.dashboardCard} ${styles.assetCard}`}
                role="button"
                tabIndex={0}
                onClick={() => {
                  void navigate(href)
                }}
                onKeyDown={event => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault()
                    void navigate(href)
                  }
                }}
              >
                <CardHeader className={styles.dashboardCardHeader}>
                  <div className={styles.cardTitleRow}>
                    <img
                      src={image}
                      alt=""
                      className={styles.assetImage}
                      onError={event => {
                        const imageElement = event.currentTarget
                        if (imageElement.getAttribute("src") !== TOKEN_PLACEHOLDER_IMAGE) {
                          imageElement.src = TOKEN_PLACEHOLDER_IMAGE
                        }
                      }}
                    />
                    <div>
                      <CardTitle className={styles.dashboardCardTitle}>{name}</CardTitle>
                      <CardDescription className={styles.dashboardCardDescription}>
                        {symbol}
                      </CardDescription>
                    </div>
                  </div>
                </CardHeader>
                <CardContent className={styles.dashboardCardContent}>
                  <div className={styles.assetMetaGrid}>
                    <div>
                      <span className={styles.assetMetaLabel}>Supply</span>
                      <span className={styles.assetMetaValue}>{formatTokenSupply(token)}</span>
                    </div>
                    <div>
                      <span className={styles.assetMetaLabel}>Mintable</span>
                      <span className={styles.assetMetaValue}>{token.mintable ? "Yes" : "No"}</span>
                    </div>
                  </div>
                </CardContent>
              </Card>
            )
          })
        )}
      </section>
    </>
  )
}

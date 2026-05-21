import * as React from "react"
import {Card, CardContent, CardDescription, CardHeader, CardTitle} from "@acton/shared-ui"
import {useNavigate} from "react-router-dom"

import type {TonClient} from "../../explorer/api/client"
import type {NftItem} from "../../explorer/api/types"
import {NFT_PLACEHOLDER_IMAGE} from "../constants"
import {contentString} from "../dashboardUtils"

import styles from "../DashboardPage.module.css"

interface NftsPageProps {
  readonly client: TonClient
}

interface NftsState {
  readonly items: readonly NftItem[]
  readonly isLoading: boolean
  readonly error?: string
}

export const NftsPage: React.FC<NftsPageProps> = ({client}) => {
  const navigate = useNavigate()
  const [nftsState, setNftsState] = React.useState<NftsState>({
    items: [],
    isLoading: true,
  })

  React.useEffect(() => {
    let cancelled = false

    void (async () => {
      setNftsState({
        items: [],
        isLoading: true,
      })

      try {
        const nfts = await client.getNftItems({
          limit: 100,
          offset: 0,
          sortByLastTransactionLt: true,
        })
        if (cancelled) {
          return
        }
        setNftsState({
          items: nfts,
          isLoading: false,
        })
      } catch (error) {
        if (cancelled) {
          return
        }
        setNftsState({
          items: [],
          isLoading: false,
          error: error instanceof Error ? error.message : "Failed to load NFTs",
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
          <h1 className={styles.title}>NFTs</h1>
          <p className={styles.subtitle}>NFT items indexed from the local network.</p>
        </div>
      </section>

      <section className={styles.resourceGrid}>
        {nftsState.error ? (
          <div className={styles.emptyState}>{nftsState.error}</div>
        ) : nftsState.isLoading ? (
          <div className={styles.emptyState}>Loading NFTs…</div>
        ) : nftsState.items.length === 0 ? (
          <div className={styles.emptyState}>No NFTs yet.</div>
        ) : (
          nftsState.items.map(item => {
            const name = contentString(item.content, "name") || "NFT Item"
            const image = contentString(item.content, "image") || NFT_PLACEHOLDER_IMAGE
            const collectionName =
              contentString(item.collection?.collection_content, "name") || "Standalone"
            const href = `/explorer/address/${encodeURIComponent(item.address)}`

            return (
              <Card
                key={item.address}
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
                        if (imageElement.getAttribute("src") !== NFT_PLACEHOLDER_IMAGE) {
                          imageElement.src = NFT_PLACEHOLDER_IMAGE
                        }
                      }}
                    />
                    <div>
                      <CardTitle className={styles.dashboardCardTitle}>{name}</CardTitle>
                      <CardDescription className={styles.dashboardCardDescription}>
                        #{item.index}
                      </CardDescription>
                    </div>
                  </div>
                </CardHeader>
                <CardContent className={styles.dashboardCardContent}>
                  <div className={styles.assetMetaGrid}>
                    <div>
                      <span className={styles.assetMetaLabel}>Collection</span>
                      <span className={styles.assetMetaValue}>{collectionName}</span>
                    </div>
                    <div>
                      <span className={styles.assetMetaLabel}>Sale</span>
                      <span className={styles.assetMetaValue}>
                        {item.on_sale ? "Listed" : "Not listed"}
                      </span>
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

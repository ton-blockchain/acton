import {Card, CardContent, CardDescription, CardHeader, CardTitle} from "@acton/shared-ui"
import type {FC, JSX} from "react"
import {useEffect, useState} from "react"
import {useNavigate} from "react-router-dom"

import type {TonClient} from "../../explorer/api/client"
import type {NftItem} from "../../explorer/api/types"
import {useDelayedLoadingVisibility} from "../../hooks/useDelayedLoadingVisibility"
import {NFT_PLACEHOLDER_IMAGE} from "../constants"
import styles from "../DashboardPage.module.css"
import {contentString} from "../dashboardUtils"

interface NftsPageProps {
  readonly client: TonClient
}

interface NftsState {
  readonly items: readonly NftItem[]
  readonly isLoading: boolean
  readonly error?: string
}

export const NftsPage: FC<NftsPageProps> = ({client}) => {
  const navigate = useNavigate()
  const [nftsState, setNftsState] = useState<NftsState>({
    items: [],
    isLoading: true,
  })
  const showLoadingSkeleton = useDelayedLoadingVisibility(nftsState.isLoading, 500)

  useEffect(() => {
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

      <section
        className={styles.resourceGrid}
        aria-busy={nftsState.isLoading}
        aria-label={nftsState.isLoading ? "Loading NFTs" : undefined}
      >
        {nftsState.error ? (
          <div className={styles.emptyState}>{nftsState.error}</div>
        ) : nftsState.isLoading ? (
          showLoadingSkeleton ? (
            <NftCardsSkeleton />
          ) : null
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

function NftCardsSkeleton(): JSX.Element {
  return (
    <>
      {Array.from({length: 3}, (_, index) => (
        <Card
          key={`nft-card-skeleton-${index}`}
          className={`${styles.dashboardCard} ${styles.assetCard} ${styles.assetSkeletonCard}`}
          aria-hidden="true"
        >
          <CardHeader className={styles.dashboardCardHeader}>
            <div className={styles.cardTitleRow}>
              <span className={`${styles.skeletonAvatar} ${styles.assetImageSkeleton}`} />
              <div className={styles.assetSkeletonTitleGroup}>
                <span className={`${styles.skeletonLine} ${styles.nftSkeletonTitle}`} />
                <span className={`${styles.skeletonLine} ${styles.nftSkeletonIndex}`} />
              </div>
            </div>
          </CardHeader>
          <CardContent className={styles.dashboardCardContent}>
            <div className={styles.assetMetaGrid}>
              <div>
                <span className={`${styles.skeletonLine} ${styles.nftSkeletonCollectionLabel}`} />
                <span className={`${styles.skeletonLine} ${styles.nftSkeletonCollectionValue}`} />
              </div>
              <div>
                <span className={`${styles.skeletonLine} ${styles.nftSkeletonSaleLabel}`} />
                <span className={`${styles.skeletonLine} ${styles.nftSkeletonSaleValue}`} />
              </div>
            </div>
          </CardContent>
        </Card>
      ))}
    </>
  )
}

import {
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
} from "@acton/ui"
import {useNavigate} from "react-router-dom"
import {useEffect, useState} from "react"
import type {FC} from "react"

import type {TonClient} from "../../explorer/api/client"
import type {NftItem} from "../../explorer/api/types"
import {useDelayedLoadingVisibility} from "../../hooks/useDelayedLoadingVisibility"
import {AddressChip} from "../../explorer/components/AddressChip"
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
          <p className={styles.subtitle}>NFT items indexed from the local network</p>
        </div>
      </section>

      <section
        className={styles.resourceTableLayout}
        aria-busy={nftsState.isLoading}
        aria-label={nftsState.isLoading ? "Loading NFTs" : undefined}
      >
        <DataTable minWidth="54rem">
          <DataTableTable aria-label="NFTs" layout="fixed">
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell>NFT</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="8rem">Index</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="14rem">Collection</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="8rem">Sale</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="17rem">Address</DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {nftsState.error ? (
                <DataTableEmpty colSpan={5}>{nftsState.error}</DataTableEmpty>
              ) : nftsState.isLoading ? (
                showLoadingSkeleton ? (
                  <DataTableSkeletonRows
                    columns={5}
                    rows={3}
                    widths={["13rem", "4rem", "10rem", "5rem", "14rem"]}
                    rowKeyPrefix="nft-table-skeleton"
                  />
                ) : null
              ) : nftsState.items.length === 0 ? (
                <DataTableEmpty colSpan={5}>No NFTs yet</DataTableEmpty>
              ) : (
                nftsState.items.map(item => {
                  const name = contentString(item.content, "name") || "NFT Item"
                  const image = contentString(item.content, "image") || NFT_PLACEHOLDER_IMAGE
                  const collectionName =
                    contentString(item.collection?.collection_content, "name") || "Standalone"
                  const href = `/explorer/address/${encodeURIComponent(item.address)}`

                  return (
                    <DataTableRow
                      key={item.address}
                      interactive
                      tabIndex={0}
                      onClick={() => {
                        void navigate(href)
                      }}
                      onKeyDown={event => {
                        if (event.target !== event.currentTarget) return
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault()
                          void navigate(href)
                        }
                      }}
                    >
                      <DataTableCell>
                        <div className={styles.assetTableIdentity}>
                          <img
                            src={image}
                            alt=""
                            className={styles.assetTableImage}
                            onError={event => {
                              const imageElement = event.currentTarget
                              if (imageElement.getAttribute("src") !== NFT_PLACEHOLDER_IMAGE) {
                                imageElement.src = NFT_PLACEHOLDER_IMAGE
                              }
                            }}
                          />
                          <strong className={styles.assetTableName}>{name}</strong>
                        </div>
                      </DataTableCell>
                      <DataTableCell tone="muted">#{item.index}</DataTableCell>
                      <DataTableCell truncate title={collectionName}>
                        {collectionName}
                      </DataTableCell>
                      <DataTableCell>
                        <span
                          className={
                            item.on_sale
                              ? styles.assetTableStatusPositive
                              : styles.assetTableStatusMuted
                          }
                        >
                          {item.on_sale ? "Listed" : "Not listed"}
                        </span>
                      </DataTableCell>
                      <DataTableCell>
                        <AddressChip address={item.address} resolveName={false} />
                      </DataTableCell>
                    </DataTableRow>
                  )
                })
              )}
            </DataTableBody>
          </DataTableTable>
        </DataTable>
      </section>
    </>
  )
}

import {Search} from "lucide-react"
import {useMemo, useState} from "react"
import type {FC} from "react"

import type {NftItem} from "../api/types"
import type {ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"

import {NftImage} from "./NftImage"
import {NFT_CARD_IMAGE_SOURCE_KEYS, getNftImageSources} from "./imageFallbacks"
import styles from "./Nfts.module.css"

interface NftsProps {
  readonly items: NftItem[]
  readonly emptyLabel?: string
  readonly searchLabel?: string
  readonly onAddressClick?: (addr: string, event?: ExplorerNavigationClickEvent) => void
}

const getContentString = (content: Record<string, unknown>, key: string): string | undefined => {
  const value = content[key]
  return typeof value === "string" && value.length > 0 ? value : undefined
}

function getCollectionName(item: NftItem): string | undefined {
  return (
    getContentString(item.content, "collection_name") ||
    getContentString(item.content, "collection")
  )
}

function getNftDisplayName(item: NftItem): string {
  const collectionName = getCollectionName(item)
  return getContentString(item.content, "name") || `${collectionName || "NFT"} #${item.index}`
}

export const Nfts: FC<NftsProps> = ({
  items,
  emptyLabel = "No NFTs yet",
  searchLabel = "Search collectibles",
  onAddressClick,
}) => {
  const [query, setQuery] = useState("")
  const normalizedQuery = query.trim().toLowerCase()
  const visibleItems = useMemo(() => {
    if (!normalizedQuery) return items

    return items.filter(item => {
      const name = getNftDisplayName(item)
      const collectionName = getCollectionName(item) || item.collection_address || ""
      const searchable = [
        name,
        collectionName,
        item.address,
        item.collection_address,
        item.owner_address,
        String(item.index),
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase()

      return searchable.includes(normalizedQuery)
    })
  }, [items, normalizedQuery])

  if (items.length === 0) {
    return <div className={styles.empty}>{emptyLabel}</div>
  }

  return (
    <div className={styles.container}>
      <label className={styles.searchBox}>
        <Search size={16} aria-hidden="true" />
        <input
          value={query}
          onChange={event => setQuery(event.target.value)}
          placeholder="Search"
          aria-label={searchLabel}
        />
      </label>
      <div className={styles.list}>
        {visibleItems.map(item => {
          const name = getNftDisplayName(item)
          const collectionName = getCollectionName(item)
          const imageSources =
            item.is_nsfw === true
              ? []
              : getNftImageSources(item.content, NFT_CARD_IMAGE_SOURCE_KEYS)
          const isScam = item.is_scam === true

          return (
            <div
              key={item.address}
              className={styles.nftItem}
              onClick={event => onAddressClick?.(item.address, event)}
              onKeyDown={event => {
                if (event.key === "Enter" || event.key === " ") {
                  onAddressClick?.(item.address)
                }
              }}
              role="button"
              tabIndex={0}
            >
              <div className={styles.imageFrame}>
                <NftImage
                  sources={imageSources}
                  alt={name}
                  className={styles.nftImage}
                  blurredClassName={styles.blurredImage}
                  blurred={isScam}
                />
                {isScam && <span className={styles.scamLabel}>SCAM</span>}
              </div>
              <div className={styles.nftInfo}>
                <div className={styles.collectionName} title={collectionName}>
                  {collectionName ||
                    (item.collection_address || item.collection?.address
                      ? "Unknown collection"
                      : "No collection")}
                </div>
                <div className={styles.nftName} title={name}>
                  {name}
                </div>
              </div>
            </div>
          )
        })}
      </div>
      {visibleItems.length === 0 && <div className={styles.empty}>No matching NFTs</div>}
    </div>
  )
}

export const NftsSkeleton: FC = () => (
  <div className={styles.container} aria-label="Loading collection items">
    <div className={`${styles.searchBox} ${styles.skeleton}`} />
    <div className={styles.list}>
      {Array.from({length: 6}, (_, index) => (
        <div key={index} className={styles.nftItem} aria-hidden="true">
          <div className={`${styles.imageFrame} ${styles.skeleton}`} />
          <div className={styles.nftInfo}>
            <div className={`${styles.skeletonLine} ${styles.skeletonLineShort}`} />
            <div className={styles.skeletonLine} />
            <div className={`${styles.skeletonLine} ${styles.skeletonLineMedium}`} />
          </div>
        </div>
      ))}
    </div>
  </div>
)

import {Search} from "lucide-react"
import {useMemo, useState} from "react"
import type {FC} from "react"

import type {NftItem} from "../api/types"
import type {ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"

import {AddressLabel} from "./AddressLabel"
import {
  NFT_IMAGE_SOURCE_KEYS,
  getImageSources,
  getPrimaryImageSource,
  replaceBrokenImageWithFallback,
} from "./imageFallbacks"
import styles from "./Nfts.module.css"

interface NftsProps {
  readonly items: NftItem[]
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

export const Nfts: FC<NftsProps> = ({items, onAddressClick}) => {
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
    return <div className={styles.empty}>No NFTs found</div>
  }

  return (
    <div className={styles.container}>
      <label className={styles.searchBox}>
        <Search size={16} aria-hidden="true" />
        <input
          value={query}
          onChange={event => setQuery(event.target.value)}
          placeholder="Search"
          aria-label="Search collectibles"
        />
      </label>
      <div className={styles.list}>
        {visibleItems.map(item => {
          const name = getNftDisplayName(item)
          const collectionName = getCollectionName(item)
          const imageSources = getImageSources(item.content, NFT_IMAGE_SOURCE_KEYS)
          const image = getPrimaryImageSource(item.content, NFT_IMAGE_SOURCE_KEYS)

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
                <img
                  src={image}
                  alt={name}
                  className={styles.nftImage}
                  onError={event => replaceBrokenImageWithFallback(event, imageSources)}
                />
              </div>
              <div className={styles.nftInfo}>
                {collectionName && <div className={styles.collectionName}>{collectionName}</div>}
                <div className={styles.nftName}>{name}</div>
                <div className={styles.nftMetaLine}>
                  <span>#{item.index}</span>
                  <span className={styles.nftAddress}>
                    <AddressLabel address={item.address} />
                  </span>
                </div>
              </div>
            </div>
          )
        })}
      </div>
      {visibleItems.length === 0 && <div className={styles.empty}>No matching collectibles</div>}
    </div>
  )
}

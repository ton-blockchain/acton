import React from "react"

import type {NftItem} from "../api/types"

import {AddressLabel} from "./AddressLabel"
import styles from "./Nfts.module.css"

interface NftsProps {
  readonly items: NftItem[]
  readonly onAddressClick?: (addr: string) => void
}

const NFT_PLACEHOLDER_IMAGE = "https://wallet.ton.org/assets/img/token-placeholder.svg"

const getContentString = (content: Record<string, unknown>, key: string): string | undefined => {
  const value = content[key]
  return typeof value === "string" && value.length > 0 ? value : undefined
}

export const Nfts: React.FC<NftsProps> = ({items, onAddressClick}) => {
  if (items.length === 0) {
    return <div className={styles.empty}>No NFTs found.</div>
  }

  return (
    <div className={styles.container}>
      <div className={styles.list}>
        {items.map(item => {
          const name = getContentString(item.content, "name") || `NFT #${item.index}`
          const description = getContentString(item.content, "description")
          const image =
            getContentString(item.content, "image") ||
            getContentString(item.content, "preview") ||
            getContentString(item.content, "image_url") ||
            NFT_PLACEHOLDER_IMAGE

          return (
            <div
              key={item.address}
              className={styles.nftItem}
              onClick={() => onAddressClick?.(item.address)}
              onKeyDown={event => {
                if (event.key === "Enter" || event.key === " ") {
                  onAddressClick?.(item.address)
                }
              }}
              role="button"
              tabIndex={0}
            >
              <div className={styles.cardHeader}>
                <div className={styles.imageFrame}>
                  <img
                    src={image}
                    alt={name}
                    className={styles.nftImage}
                    onError={event => {
                      ;(event.target as HTMLImageElement).src = NFT_PLACEHOLDER_IMAGE
                    }}
                  />
                </div>
                <div className={styles.nftInfo}>
                  <div className={styles.nftName}>{name}</div>
                  <div className={styles.nftIndex}>#{item.index}</div>
                </div>
              </div>
              {description && <div className={styles.nftDescription}>{description}</div>}
              <div className={styles.metaGrid}>
                <div className={styles.metaRow}>
                  <span className={styles.metaLabel}>Address</span>
                  <span className={`${styles.metaValue} ${styles.addressValue}`}>
                    <AddressLabel address={item.address} />
                  </span>
                </div>
                <div className={styles.metaRow}>
                  <span className={styles.metaLabel}>Collection</span>
                  <span className={`${styles.metaValue} ${styles.addressValue}`}>
                    {item.collection_address ? (
                      <AddressLabel address={item.collection_address} />
                    ) : (
                      "Unknown collection"
                    )}
                  </span>
                </div>
                <div className={styles.metaRow}>
                  <span className={styles.metaLabel}>Owner</span>
                  <span className={`${styles.metaValue} ${styles.addressValue}`}>
                    {item.owner_address ? (
                      <AddressLabel address={item.owner_address} />
                    ) : (
                      "No owner"
                    )}
                  </span>
                </div>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}

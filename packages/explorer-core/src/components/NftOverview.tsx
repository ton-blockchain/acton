import type {FC} from "react"

import {AccountAddressDetailRow, AccountDetailRows, AccountTextDetailRow} from "./AccountDetailRows"
import {NftImage} from "./NftImage"

import styles from "./NftOverview.module.css"

interface NftOverviewBaseProps {
  readonly name: string
  readonly description?: string
  readonly imageSources: readonly string[]
  readonly isScam: boolean
  readonly onAddressClick: (address: string) => void
}

interface NftItemOverviewProps extends NftOverviewBaseProps {
  readonly kind: "item"
  readonly ownerAddress?: string
  readonly collectionAddress?: string
  readonly collectionName?: string
  readonly index: string
  readonly onMetadataClick: () => void
  readonly onNsfw: () => void
}

interface NftCollectionOverviewProps extends NftOverviewBaseProps {
  readonly kind: "collection"
  readonly latestItemAddress?: string
}

export type NftOverviewProps = NftItemOverviewProps | NftCollectionOverviewProps

export const NftOverview: FC<NftOverviewProps> = props => (
  <div className={styles.card} data-account-context-card="nft">
    <div className={styles.header}>
      <div className={styles.heading}>
        <div className={styles.title}>{props.name}</div>
        {props.kind === "item" && (
          <button type="button" className={styles.metadataButton} onClick={props.onMetadataClick}>
            Metadata
          </button>
        )}
      </div>
    </div>
    <div className={styles.divider} />
    <div className={styles.body}>
      <div className={styles.main}>
        {props.kind === "item" ? (
          <AccountDetailRows>
            <AccountAddressDetailRow
              label="Owner"
              address={props.ownerAddress}
              fallback="No owner"
              onAddressClick={props.onAddressClick}
            />
            <AccountAddressDetailRow
              label="Collection Address"
              address={props.collectionAddress}
              fallback="Standalone"
              onAddressClick={props.onAddressClick}
            />
            <AccountTextDetailRow label="Index" value={`#${props.index}`} />
          </AccountDetailRows>
        ) : (
          props.latestItemAddress !== undefined && (
            <AccountDetailRows>
              <AccountAddressDetailRow
                label="Latest item"
                address={props.latestItemAddress}
                onAddressClick={props.onAddressClick}
              />
            </AccountDetailRows>
          )
        )}
        {props.description !== undefined && props.description.length > 0 && (
          <div className={styles.description}>{props.description}</div>
        )}
      </div>
      <div className={styles.media}>
        <NftImage
          sources={props.imageSources}
          alt={props.name}
          className={styles.image}
          blurredClassName={styles.blurredImage}
          collectionName={props.kind === "item" ? props.collectionName : props.name}
          blurred={props.isScam}
          onNsfw={props.kind === "item" ? props.onNsfw : undefined}
        />
        {props.isScam && <span className={styles.scamLabel}>SCAM</span>}
      </div>
    </div>
  </div>
)

import {ExternalLink} from "lucide-react"
import type {FC, ReactNode} from "react"

import {AccountAddressDetailRow, AccountDetailRows, AccountTextDetailRow} from "./AccountDetailRows"
import {NftImage} from "./NftImage"

import styles from "./NftOverview.module.css"

interface NftOverviewBaseProps {
  readonly name: string
  readonly description?: string
  readonly imageSources: readonly string[]
  readonly isScam: boolean
  readonly blurred?: boolean
  readonly revealControl?: ReactNode
  readonly onAddressClick: (address: string) => void
}

interface NftItemOverviewProps extends NftOverviewBaseProps {
  readonly kind: "item"
  readonly collectionName?: string
  readonly ownerAddress?: string
  readonly collectionAddress?: string
  readonly index: string
  readonly externalLink?: {
    readonly href: string
    readonly label: string
  }
  readonly onMetadataClick: () => void
}

interface NftCollectionOverviewProps extends NftOverviewBaseProps {
  readonly kind: "collection"
  readonly latestItemAddress?: string
}

export type NftOverviewProps = NftItemOverviewProps | NftCollectionOverviewProps

export const NftOverview: FC<NftOverviewProps> = props => (
  <div className={styles.card} data-account-context-card="nft">
    {props.imageSources.length > 0 && (
      <NftImage
        sources={props.imageSources}
        alt=""
        aria-hidden="true"
        className={styles.backgroundImage}
        blurredClassName={styles.backgroundImage}
      />
    )}
    <div className={styles.header}>
      <div className={styles.heading}>
        <div className={styles.title}>{props.name}</div>
        {props.kind === "item" && props.collectionName && (
          <div className={styles.collectionName}>
            from{" "}
            {props.collectionAddress ? (
              <button
                type="button"
                className={styles.collectionLink}
                onClick={() => {
                  if (props.collectionAddress) props.onAddressClick(props.collectionAddress)
                }}
              >
                {props.collectionName}
              </button>
            ) : (
              props.collectionName
            )}
          </div>
        )}
        {props.kind === "item" && (
          <div className={styles.actions}>
            <button type="button" className={styles.action} onClick={props.onMetadataClick}>
              Metadata
            </button>
            {props.externalLink && (
              <a
                className={styles.action}
                href={props.externalLink.href}
                target="_blank"
                rel="noreferrer"
              >
                {props.externalLink.label} <ExternalLink size={12} aria-hidden="true" />
              </a>
            )}
          </div>
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
              label="Collection address"
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
          blurred={props.blurred ?? props.isScam}
          revealControl={props.revealControl}
        />
        {props.isScam && <span className={styles.scamLabel}>SCAM</span>}
      </div>
    </div>
  </div>
)

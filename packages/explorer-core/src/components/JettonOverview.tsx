import type {FC} from "react"

import {Button, TokenAmount} from "@acton/ui"

import {replaceBrokenImageWithFallback} from "./imageFallbacks"
import {AccountAddressDetailRow, AccountDetailRows} from "./AccountDetailRows"

import styles from "./JettonOverview.module.css"

interface JettonOverviewProps {
  readonly name: string
  readonly symbol?: string
  readonly image: string
  readonly imageSources: readonly string[]
  readonly decimals?: string
  readonly totalSupply?: string
  readonly masterAddress?: string
  readonly holderAddress?: string
  readonly onAddressClick: (address: string) => void
  readonly onMetadataClick?: () => void
  readonly onMint?: () => void
}

export const JettonOverview: FC<JettonOverviewProps> = ({
  name,
  symbol,
  image,
  imageSources,
  decimals,
  totalSupply,
  masterAddress,
  holderAddress,
  onAddressClick,
  onMetadataClick,
  onMint,
}) => (
  <div className={styles.card} data-account-context-card="jetton">
    <div className={styles.header}>
      <img
        src={image}
        alt={name}
        className={styles.image}
        onError={event => replaceBrokenImageWithFallback(event, imageSources)}
      />
      <div className={styles.headerContent}>
        <div className={styles.title}>
          <div className={styles.name}>{name}</div>
          {symbol !== undefined && symbol.length > 0 && (
            <div className={styles.symbol}>{symbol}</div>
          )}
        </div>
        {totalSupply !== undefined && (
          <div className={styles.supply}>
            Max.supply:{" "}
            <TokenAmount decimals={decimals} symbol={symbol} useGrouping value={totalSupply} />
          </div>
        )}
        {onMetadataClick !== undefined && (
          <button type="button" className={styles.metadataButton} onClick={onMetadataClick}>
            Metadata
          </button>
        )}
      </div>
    </div>
    {onMint !== undefined && (
      <Button
        type="button"
        variant="outline"
        size="sm"
        className={styles.mintButton}
        onClick={onMint}
      >
        Mint token
      </Button>
    )}
    {masterAddress !== undefined && holderAddress !== undefined && (
      <>
        <div className={styles.divider} />
        <AccountDetailRows>
          <AccountAddressDetailRow
            label="Jetton master"
            address={masterAddress}
            onAddressClick={onAddressClick}
          />
          <AccountAddressDetailRow
            label="Holder address"
            address={holderAddress}
            onAddressClick={onAddressClick}
          />
        </AccountDetailRows>
      </>
    )}
  </div>
)

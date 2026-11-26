import type {FC} from "react"

import {Button, TokenAmount} from "@acton/ui"

import {replaceBrokenImageWithFallback} from "./imageFallbacks"
import {AccountAddressDetailRow, AccountDetailRows} from "./AccountDetailRows"

import styles from "./JettonOverview.module.css"

interface JettonOverviewProps {
  readonly name: string
  readonly symbol?: string
  readonly isScam: boolean
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
  isScam,
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
    {imageSources.length > 0 && (
      <img
        src={image}
        alt=""
        aria-hidden="true"
        className={styles.backgroundImage}
        onError={event => replaceBrokenImageWithFallback(event, imageSources)}
      />
    )}
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
    {isScam && <span className={styles.scamLabel}>SCAM</span>}
    {totalSupply !== undefined && (
      <div className={styles.chartPlaceholder}>
        <svg viewBox="0 0 392 112" preserveAspectRatio="none" aria-hidden="true">
          <path className={styles.chartGrid} d="M0 16H392 M0 56H392 M0 96H392" />
          <path
            className={styles.chartLine}
            d="M0 83 L18 78 L32 81 L48 69 L65 74 L83 63 L101 84 L119 76 L135 58 L153 62 L170 48 L189 71 L207 66 L223 76 L241 52 L258 60 L277 47 L294 68 L312 73 L329 56 L346 63 L365 49 L381 57 L392 53"
          />
        </svg>
        <span className={styles.chartUnavailable}>No chart data</span>
      </div>
    )}
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

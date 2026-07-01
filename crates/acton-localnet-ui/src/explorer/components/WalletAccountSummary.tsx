import {formatUnits} from "@ton/walletkit"
import type {FC} from "react"

import type {JettonMasterMetadata, JettonWallet} from "../api/types"
import type {ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"

import {
  getImageSources,
  getPrimaryImageSource,
  replaceBrokenImageWithFallback,
  TOKEN_IMAGE_SOURCE_KEYS,
} from "./imageFallbacks"
import styles from "./WalletAccountSummary.module.css"

export interface AccountBalanceState {
  readonly value?: string
  readonly isLoading: boolean
  readonly error?: string
}

interface WalletAccountSummaryProps {
  readonly address: string
  readonly tokens: readonly JettonWallet[]
  readonly tokensLoading: boolean
  readonly balanceState: AccountBalanceState | undefined
  readonly onOpenTokens: (address: string, event?: ExplorerNavigationClickEvent) => void
}

const TOKEN_PREVIEW_LIMIT = 5

export const WalletAccountSummary: FC<WalletAccountSummaryProps> = ({
  address,
  tokens,
  tokensLoading,
  balanceState,
  onOpenTokens,
}) => (
  <div className={styles.balanceGroup}>
    <WalletTokenPreview
      address={address}
      tokens={tokens}
      loading={tokensLoading}
      onOpenTokens={onOpenTokens}
    />
    <span className={styles.balance}>{formatWalletBalanceLabel(balanceState)}</span>
  </div>
)

function WalletTokenPreview({
  address,
  tokens,
  loading,
  onOpenTokens,
}: {
  readonly address: string
  readonly tokens: readonly JettonWallet[]
  readonly loading: boolean
  readonly onOpenTokens: (address: string, event?: ExplorerNavigationClickEvent) => void
}) {
  if (loading && tokens.length === 0) {
    return <span className={styles.tokenPreviewSkeleton} aria-label="Loading tokens" />
  }

  if (tokens.length === 0) {
    return null
  }

  const firstToken = tokens[0]
  const firstMaster = firstToken?.master
  const firstSymbol = firstMaster?.jetton_content.symbol || "tokens"
  const firstDecimals = parseJettonDecimals(firstMaster)
  const firstImageSources = getImageSources(firstMaster?.jetton_content, TOKEN_IMAGE_SOURCE_KEYS)
  const firstImage = getPrimaryImageSource(firstMaster?.jetton_content, TOKEN_IMAGE_SOURCE_KEYS)
  const previewTokens = tokens.slice(1, TOKEN_PREVIEW_LIMIT)

  return (
    <button
      type="button"
      className={styles.tokenPreviewButton}
      onClick={event => onOpenTokens(address, event)}
      title="Open wallet tokens"
      aria-label="Open wallet tokens"
    >
      <img
        src={firstImage}
        alt=""
        className={styles.tokenPreviewIcon}
        onError={event => replaceBrokenImageWithFallback(event, firstImageSources)}
      />
      <span className={styles.tokenPreviewAmount}>
        {formatTokenAmount(firstToken.balance, firstDecimals)} {firstSymbol}
      </span>
      {previewTokens.length > 0 && (
        <span className={styles.tokenPreviewStack} aria-hidden="true">
          {previewTokens.map((token, index) => {
            const imageSources = getImageSources(
              token.master?.jetton_content,
              TOKEN_IMAGE_SOURCE_KEYS,
            )
            const image = imageSources[0]
            return image ? (
              <img
                key={token.address}
                src={image}
                alt=""
                className={styles.tokenPreviewStackIcon}
                style={{zIndex: previewTokens.length - index}}
                onError={event => replaceBrokenImageWithFallback(event, imageSources)}
              />
            ) : (
              <span
                key={token.address}
                className={styles.tokenPreviewStackPlaceholder}
                style={{zIndex: previewTokens.length - index}}
              />
            )
          })}
        </span>
      )}
      <span className={styles.tokenPreviewAction}>View all</span>
    </button>
  )
}

function formatWalletBalanceLabel(balanceState: AccountBalanceState | undefined): string {
  if (!balanceState) {
    return "Loading balance..."
  }

  if (balanceState.value) {
    const balance = `${formatCompactGramBalance(balanceState.value)} GRAM`
    return balanceState.isLoading ? `${balance} · updating` : balance
  }

  if (balanceState.isLoading) {
    return "Loading balance..."
  }

  return balanceState.error ? "Balance unavailable" : "Balance not loaded"
}

function formatCompactGramBalance(balance: string): string {
  const formattedBalance = formatUnits(balance, 9)
  const numericBalance = Number(formattedBalance)

  if (!Number.isFinite(numericBalance)) {
    return formattedBalance
  }

  if (numericBalance > 0 && numericBalance < 0.0001) {
    return "<0.0001"
  }

  return numericBalance.toLocaleString(undefined, {
    maximumFractionDigits: 4,
  })
}

function parseJettonDecimals(master: JettonMasterMetadata | undefined): number {
  const decimals = Number(master?.jetton_content.decimals)
  return Number.isFinite(decimals) ? decimals : 9
}

function formatTokenAmount(value: string, decimals: number): string {
  const decimalsNumber = Number.isFinite(decimals) ? decimals : 9
  return (Number(value) / 10 ** decimalsNumber).toLocaleString(undefined, {
    maximumFractionDigits: decimalsNumber,
  })
}

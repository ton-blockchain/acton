import {ArrowDownLeft, ArrowUpRight, Blocks, ExternalLink, RefreshCw} from "lucide-react"
import {useCallback, useEffect, useState} from "react"

import {
  formatTonBalance,
  getTransactionExplorerUrl,
  shortenAddress,
  type WalletRecord,
} from "../domain/wallet"
import {
  fetchWalletActivity,
  type WalletActivityItem,
  type WalletActivityDirection,
} from "../services/walletActivity"
import {openExternalUrl} from "../services/externalLinks"
import styles from "./WalletActivity.module.css"

interface WalletActivityProps {
  readonly wallet: WalletRecord
  readonly refreshToken: number
}

interface ActivityState {
  readonly items: readonly WalletActivityItem[]
  readonly isLoading: boolean
  readonly error?: string
}

const ACTIVITY_COPY: Record<
  WalletActivityDirection,
  {readonly label: string; readonly counterparty: string}
> = {
  incoming: {label: "Received", counterparty: "From"},
  outgoing: {label: "Sent", counterparty: "To"},
  contract: {label: "Contract call", counterparty: "Account"},
}

export function WalletActivity({wallet, refreshToken}: WalletActivityProps) {
  const [state, setState] = useState<ActivityState>({items: [], isLoading: true})

  const loadActivity = useCallback(
    async (force = false) => {
      setState(current => ({...current, isLoading: true, error: undefined}))
      try {
        setState({items: await fetchWalletActivity(wallet, force), isLoading: false})
      } catch (error) {
        setState(current => ({
          items: current.items,
          isLoading: false,
          error: getErrorMessage(error),
        }))
      }
    },
    [wallet],
  )

  useEffect(() => {
    void loadActivity(refreshToken > 0)
  }, [loadActivity, refreshToken])

  return (
    <section className={styles.activity}>
      <header className={styles.header}>
        <div>
          <span>03</span>
          <h2>Recent activity</h2>
        </div>
        <button
          type="button"
          aria-label="Refresh activity"
          title="Refresh activity"
          disabled={state.isLoading}
          onClick={() => void loadActivity(true)}
        >
          <RefreshCw size={16} className={state.isLoading ? styles.spinning : undefined} />
        </button>
      </header>

      {state.isLoading && state.items.length === 0 ? (
        <div className={styles.loading} aria-label="Loading recent activity">
          <span />
          <span />
          <span />
        </div>
      ) : undefined}
      {state.error ? <p className={styles.error}>{state.error}</p> : undefined}
      {!(state.isLoading || state.error) && state.items.length === 0 ? (
        <p className={styles.empty}>No transactions found for this account yet.</p>
      ) : undefined}

      <div className={styles.list}>
        {state.items.map(item => (
          <ActivityRow key={`${item.hash}:${item.direction}`} wallet={wallet} item={item} />
        ))}
      </div>
    </section>
  )
}

function ActivityRow({
  wallet,
  item,
}: {
  readonly wallet: WalletRecord
  readonly item: WalletActivityItem
}) {
  const copy = ACTIVITY_COPY[item.direction]
  const amountPrefix = getAmountPrefix(item.direction)
  const href = getTransactionExplorerUrl(wallet, item.hash)

  return (
    <a
      className={styles.row}
      href={href}
      target="_blank"
      rel="noreferrer"
      onClick={event => {
        event.preventDefault()
        void openExternalUrl(href)
      }}
    >
      <span className={styles.direction} data-direction={item.direction}>
        <DirectionIcon direction={item.direction} />
      </span>
      <span className={styles.details}>
        <strong>{copy.label}</strong>
        <small>
          {item.counterparty
            ? `${copy.counterparty} ${shortenAddress(item.counterparty, 6)}`
            : "External message"}
        </small>
      </span>
      <span className={styles.time}>
        <time dateTime={new Date(item.timestamp * 1000).toISOString()}>
          {formatActivityTime(item.timestamp)}
        </time>
        <small>Fee {formatActivityFee(item.feeNano)} GRAM</small>
      </span>
      <span className={styles.amount}>
        <strong>
          {amountPrefix}
          {formatTonBalance(item.valueNano)} GRAM
        </strong>
        <ExternalLink size={14} />
      </span>
    </a>
  )
}

function DirectionIcon({direction}: {readonly direction: WalletActivityDirection}) {
  if (direction === "incoming") return <ArrowDownLeft size={17} />
  if (direction === "outgoing") return <ArrowUpRight size={17} />
  return <Blocks size={17} />
}

function getAmountPrefix(direction: WalletActivityDirection): string {
  if (direction === "incoming") return "+"
  if (direction === "outgoing") return "−"
  return ""
}

function formatActivityTime(timestamp: number): string {
  if (!timestamp) return "Pending timestamp"
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp * 1000))
}

function formatActivityFee(valueNano: string): string {
  const value = BigInt(valueNano)
  if (value === 0n) return "0"

  const whole = value / 1_000_000_000n
  const fraction = (value % 1_000_000_000n)
    .toString()
    .padStart(9, "0")
    .replace(/0+$/, "")

  return `${whole}.${fraction}`
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message
  if (typeof error === "string" && error.trim()) return error
  return "Activity is temporarily unavailable."
}

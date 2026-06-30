import type {FC} from "react"
import {
  type ContractData,
  ContractChip,
  StorageDiffView,
  type TransactionInfo,
  buildStorageDiff,
} from "@acton/shared-ui"

import type {ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"

import InlineLoader from "../retrace/txTrace/ui/InlineLoader"
import "../retrace/Retrace.tokens.css"

import styles from "./TraceStateChangesPanel.module.css"

interface TraceStateChangesPanelProps {
  readonly transactions: readonly TransactionInfo[]
  readonly contracts: Map<string, ContractData>
  readonly isLoading?: boolean
  readonly error?: string
  readonly onContractClick: (address: string, event?: ExplorerNavigationClickEvent) => void
}

interface TraceStateChangeItem {
  readonly address: string
  readonly storageDiff: ReturnType<typeof buildStorageDiff>
  readonly hasStorageChange: boolean
}

export const TraceStateChangesPanel: FC<TraceStateChangesPanelProps> = ({
  transactions,
  contracts,
  isLoading = false,
  error,
  onContractClick,
}) => {
  const items = buildTraceStateChangeItems(transactions)

  if (isLoading) {
    return (
      <div className={`${styles.loadingState} retraceRoot`}>
        <InlineLoader
          message="Loading decoded state changes"
          subtext="Replaying the trace locally"
          loading={true}
        />
      </div>
    )
  }

  if (error && items.length === 0) {
    return <div className={styles.emptyState}>State changes unavailable: {error}</div>
  }

  if (items.length === 0) {
    return <div className={styles.emptyState}>No decoded state changes found for this trace</div>
  }

  return (
    <div className={styles.panel}>
      {error && <div className={styles.statusNote}>State changes unavailable: {error}</div>}
      {items.map(item => (
        <section key={item.address} className={styles.card}>
          <div className={styles.cardHeader}>
            <ContractChip
              address={item.address}
              contracts={contracts}
              onContractClick={onContractClick}
            />
          </div>

          {item.storageDiff && (
            <div className={styles.storageScroll}>
              <StorageDiffView
                diff={item.storageDiff}
                contracts={contracts}
                onContractClick={onContractClick}
              />
            </div>
          )}
        </section>
      ))}
    </div>
  )
}

function buildTraceStateChangeItems(
  transactions: readonly TransactionInfo[],
): readonly TraceStateChangeItem[] {
  const transactionsByAddress = new Map<string, TransactionInfo[]>()

  for (const tx of transactions) {
    const address = tx.address?.toString()
    if (!address) {
      continue
    }

    const addressTransactions = transactionsByAddress.get(address)
    if (addressTransactions) {
      addressTransactions.push(tx)
    } else {
      transactionsByAddress.set(address, [tx])
    }
  }

  return [...transactionsByAddress.entries()]
    .map(([address, addressTransactions]) => {
      const sortedTransactions = [...addressTransactions].sort(compareTraceTransactionLt)
      const firstTx = sortedTransactions[0]
      const lastTx = sortedTransactions.at(-1)
      const storageDiff = buildStorageDiff(firstTx?.parsedStorageBefore, lastTx?.parsedStorageAfter)
      const hasStorageChange = storageDiff !== undefined && storageDiff.status !== "unchanged"

      return {
        address,
        storageDiff,
        hasStorageChange,
      }
    })
    .filter(item => item.hasStorageChange)
}

function compareTraceTransactionLt(left: TransactionInfo, right: TransactionInfo): number {
  const leftLt = parseTraceLt(left.lt)
  const rightLt = parseTraceLt(right.lt)
  if (leftLt === rightLt) {
    return 0
  }
  return leftLt < rightLt ? -1 : 1
}

function parseTraceLt(value: string | undefined): bigint {
  try {
    return value === undefined ? 0n : BigInt(value)
  } catch {
    return 0n
  }
}

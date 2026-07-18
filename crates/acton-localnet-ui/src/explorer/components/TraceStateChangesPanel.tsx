import type {FC} from "react"
import {AlertCircle} from "lucide-react"
import {
  buildStorageDiff,
  ContractChip,
  InlineLoader,
  type ParsedCodeCell,
  type ParsedValueDiff,
  ParsedValueDiffView,
} from "@acton/ui"
import {
  CodeCellDetails,
  type ContractData,
  type ContractVerifiedSource,
  type ResolveVerifiedSourceByCodeHash,
  type TransactionInfo,
} from "@acton/transaction-ui"

import type {ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"

import styles from "./TraceStateChangesPanel.module.css"

interface TraceStateChangesPanelProps {
  readonly transactions: readonly TransactionInfo[]
  readonly contracts: ReadonlyMap<string, ContractData>
  readonly verifiedSourcesByCodeHash?: ReadonlyMap<string, ContractVerifiedSource>
  readonly resolveVerifiedSourceByCodeHash?: ResolveVerifiedSourceByCodeHash
  readonly isLoading?: boolean
  readonly error?: string
  readonly onContractClick: (
    address: string,
    event?: ExplorerNavigationClickEvent,
  ) => void
}

interface TraceStateChangeItem {
  readonly address: string
  readonly storageDiff: ParsedValueDiff
}

export const TraceStateChangesPanel: FC<TraceStateChangesPanelProps> = ({
  transactions,
  contracts,
  verifiedSourcesByCodeHash,
  resolveVerifiedSourceByCodeHash,
  isLoading = false,
  error,
  onContractClick,
}) => {
  const items = buildTraceStateChangeItems(transactions)

  if (isLoading) {
    return (
      <div className={styles.loadingState}>
        <InlineLoader
          message="Loading decoded state changes"
          subtext="Replaying the trace locally"
        />
      </div>
    )
  }

  if (error && items.length === 0) {
    return (
      <div className={styles.errorState} role="alert">
        <div className={styles.errorContent}>
          <AlertCircle className={styles.errorIcon} aria-hidden="true" />
          <div>
            <div className={styles.errorTitle}>State changes unavailable</div>
            <div className={styles.errorMessage}>{error}</div>
          </div>
        </div>
      </div>
    )
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
          <div className={styles.storageScroll}>
            <ParsedValueDiffView
              diff={item.storageDiff}
              contracts={contracts}
              onContractClick={onContractClick}
              renderCodeCellDetails={(cell: ParsedCodeCell) => (
                <CodeCellDetails
                  cell={cell}
                  verifiedSourcesByCodeHash={verifiedSourcesByCodeHash}
                  resolveVerifiedSourceByCodeHash={resolveVerifiedSourceByCodeHash}
                />
              )}
            />
          </div>
        </section>
      ))}
    </div>
  )
}

function buildTraceStateChangeItems(
  transactions: readonly TransactionInfo[],
): readonly TraceStateChangeItem[] {
  const transactionsByAddress = new Map<string, TransactionInfo[]>()

  for (const transaction of transactions) {
    const address = transaction.address?.toString()
    if (!address) continue

    const addressTransactions = transactionsByAddress.get(address)
    if (addressTransactions) {
      addressTransactions.push(transaction)
    } else {
      transactionsByAddress.set(address, [transaction])
    }
  }

  return [...transactionsByAddress.entries()].flatMap(([address, addressTransactions]) => {
    const sortedTransactions = addressTransactions.toSorted(compareTraceTransactionLt)
    const storageDiff = buildStorageDiff(
      sortedTransactions[0]?.parsedStorageBefore,
      sortedTransactions.at(-1)?.parsedStorageAfter,
    )

    return storageDiff && storageDiff.status !== "unchanged" ? [{address, storageDiff}] : []
  })
}

function compareTraceTransactionLt(left: TransactionInfo, right: TransactionInfo): number {
  const leftLt = parseTraceLt(left.lt)
  const rightLt = parseTraceLt(right.lt)
  return leftLt === rightLt ? 0 : leftLt < rightLt ? -1 : 1
}

function parseTraceLt(value: string | undefined): bigint {
  try {
    return value === undefined ? 0n : BigInt(value)
  } catch {
    return 0n
  }
}

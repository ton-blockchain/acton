import type React from "react"

import type {ContractData, ValueFlowItem} from "@/types/transaction"
import {formatCurrency} from "@/utils/format"

import {ContractChip} from "../ContractChip/ContractChip"

import styles from "./ValueFlowTable.module.css"

export interface ValueFlowTableProps {
  readonly items: readonly ValueFlowItem[]
  readonly contracts: Map<string, ContractData>
  readonly onContractClick?: (address: string) => void
}

export function ValueFlowTable({
  items,
  contracts,
  onContractClick,
}: ValueFlowTableProps): React.JSX.Element {
  const totalFee = items.reduce((sum, item) => sum + item.fee, 0n)
  const showTotal = items.length > 1

  return (
    <div className={styles.valueFlowContainer}>
      <div className={styles.flowList}>
        <div className={styles.flowHeader}>
          <div className={styles.flowCol}>Account</div>
          <div className={`${styles.flowCol} ${styles.amountCol}`}>Balance Change</div>
          <div className={`${styles.flowCol} ${styles.feeCol}`}>Network Fee</div>
        </div>
        {items.map(item => (
          <div key={item.address} className={styles.flowRow}>
            <div className={styles.flowCol}>
              <ContractChip
                address={item.address}
                contracts={contracts}
                onContractClick={onContractClick}
              />
            </div>
            <div
              className={`${styles.flowCol} ${styles.amountCol} ${item.change > 0n ? styles.positive : item.change < 0n ? styles.negative : ""}`}
            >
              <div className={styles.changeValue}>{formatSignedCurrency(item.change)}</div>
            </div>
            <div className={`${styles.flowCol} ${styles.feeCol}`}>{formatCurrency(item.fee)}</div>
          </div>
        ))}
        {showTotal && (
          <div className={styles.flowFooter}>
            <div className={styles.flowCol} />
            <div className={styles.flowCol} />
            <div className={`${styles.flowCol} ${styles.feeCol} ${styles.totalFee}`}>
              Total: {formatCurrency(totalFee)}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

function formatSignedCurrency(value: bigint): string {
  if (value > 0n) {
    return `+ ${formatCurrency(value)}`
  }

  if (value < 0n) {
    return `- ${formatCurrency(-value)}`
  }

  return formatCurrency(value)
}

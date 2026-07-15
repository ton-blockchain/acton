import type React from "react"
import {
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableFooter,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableTable,
} from "@acton/ui"

import type {ContractData, ValueFlowItem} from "@/types/transaction"
import {formatCurrency} from "@/utils/format"

import {ContractChip} from "../ContractChip/ContractChip"

import styles from "./ValueFlowTable.module.css"

export interface ValueFlowTableProps {
  readonly items: readonly ValueFlowItem[]
  readonly contracts: Map<string, ContractData>
  readonly onContractClick?: (address: string) => void
  readonly className?: string
}

export function ValueFlowTable({
  items,
  contracts,
  onContractClick,
  className,
}: ValueFlowTableProps): React.JSX.Element {
  const totalFee = items.reduce((sum, item) => sum + item.fee, 0n)
  const showTotal = items.length > 1
  const sortedItems = [...items].sort((left, right) => {
    const leftLetter = contracts.get(left.address)?.letter
    const rightLetter = contracts.get(right.address)?.letter

    if (leftLetter && rightLetter) {
      return leftLetter.localeCompare(rightLetter)
    }
    if (leftLetter) {
      return -1
    }
    if (rightLetter) {
      return 1
    }

    return left.address.localeCompare(right.address)
  })

  return (
    <DataTable className={className} minWidth="34rem">
      <DataTableTable aria-label="Value flow">
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell columnWidth="34%">Account</DataTableHeaderCell>
            <DataTableHeaderCell align="right" columnWidth="33%">
              Balance Change
            </DataTableHeaderCell>
            <DataTableHeaderCell align="right" columnWidth="33%">
              Network Fee
            </DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {sortedItems.map(item => (
            <DataTableRow key={item.address}>
              <DataTableCell>
                <ContractChip
                  address={item.address}
                  contracts={contracts}
                  onContractClick={onContractClick}
                />
              </DataTableCell>
              <DataTableCell align="right">
                <span className={item.change > 0n ? styles.positive : undefined}>
                  {formatSignedCurrency(item.change)}
                </span>
              </DataTableCell>
              <DataTableCell align="right">{formatCurrency(item.fee)}</DataTableCell>
            </DataTableRow>
          ))}
        </DataTableBody>
        {showTotal && (
          <DataTableFooter>
            <DataTableRow>
              <DataTableCell colSpan={2} />
              <DataTableCell align="right" className={styles.totalCell} tone="strong">
                Total: {formatCurrency(totalFee)}
              </DataTableCell>
            </DataTableRow>
          </DataTableFooter>
        )}
      </DataTableTable>
    </DataTable>
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

import type React from "react"
import {
  ContractChip,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableFooter,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableTable,
  GramAmount,
  TokenAmount,
} from "@acton/ui"

import type {ContractData, ValueFlowAsset, ValueFlowItem} from "../../model/transaction"
import {formatAddress} from "../../lib/format"

import styles from "./ValueFlowTable.module.css"

export interface ValueFlowTableProps {
  readonly items: readonly ValueFlowItem[]
  readonly contracts: Map<string, ContractData>
  readonly onContractClick?: (address: string) => void
  readonly emptyState?: string
  readonly className?: string
}

export function ValueFlowTable({
  items,
  contracts,
  onContractClick,
  emptyState = "No value flow data",
  className,
}: ValueFlowTableProps): React.JSX.Element {
  const totalFee = items.reduce((sum, item) => sum + item.fee, 0n)
  const showTotal = items.length > 1
  const assets = collectAssets(items)
  const numericColumnWidth = "12rem"
  const sortedItems = items.toSorted((left, right) => {
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
    <DataTable
      className={`${styles.root} ${className ?? ""}`}
      minWidth={`var(--value-flow-table-min-width, ${20 + (assets.length + 2) * 12}rem)`}
    >
      <DataTableTable className={styles.table} aria-label="Value flow" rowDividers={false}>
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell columnWidth="20rem">Account</DataTableHeaderCell>
            <DataTableHeaderCell align="right" columnWidth={numericColumnWidth}>
              Balance Change
            </DataTableHeaderCell>
            {assets.map(asset => (
              <DataTableHeaderCell
                key={asset.id}
                align="right"
                columnWidth={numericColumnWidth}
                title={asset.id}
              >
                {asset.symbol ?? formatAddress(asset.id)}
              </DataTableHeaderCell>
            ))}
            <DataTableHeaderCell align="right" columnWidth={numericColumnWidth}>
              Network Fee
            </DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {sortedItems.length === 0 ? (
            <DataTableEmpty colSpan={3 + assets.length}>{emptyState}</DataTableEmpty>
          ) : (
            sortedItems.map(item => (
              <DataTableRow key={item.address}>
                <DataTableCell className={styles.accountCell}>
                  <ContractChip
                    address={item.address}
                    contracts={contracts}
                    onContractClick={onContractClick}
                  />
                </DataTableCell>
                <DataTableCell align="right" data-mobile-label="Balance change">
                  <span className={item.change > 0n ? styles.positive : undefined}>
                    <GramAmount signDisplay="except-zero" value={item.change} />
                  </span>
                </DataTableCell>
                {assets.map(asset => {
                  const assetChange = item.assetChanges.find(change => change.asset.id === asset.id)
                  return (
                    <DataTableCell
                      key={asset.id}
                      align="right"
                      data-mobile-label={asset.symbol ?? formatAddress(asset.id)}
                    >
                      {assetChange && (
                        <span
                          className={
                            assetChange.change > 0n
                              ? `${styles.assetValue} ${styles.positive}`
                              : styles.assetValue
                          }
                        >
                          <TokenAmount
                            decimals={asset.decimals ?? 0}
                            signDisplay="except-zero"
                            symbol={asset.symbol}
                            value={assetChange.change}
                          />
                        </span>
                      )}
                    </DataTableCell>
                  )
                })}
                <DataTableCell align="right" data-mobile-label="Network fee">
                  <GramAmount value={item.fee} />
                </DataTableCell>
              </DataTableRow>
            ))
          )}
        </DataTableBody>
        {showTotal && (
          <DataTableFooter>
            <DataTableRow>
              <DataTableCell colSpan={2 + assets.length} />
              <DataTableCell align="right" className={styles.totalCell} tone="strong">
                Total: <GramAmount value={totalFee} />
              </DataTableCell>
            </DataTableRow>
          </DataTableFooter>
        )}
      </DataTableTable>
    </DataTable>
  )
}

function collectAssets(items: readonly ValueFlowItem[]): ValueFlowAsset[] {
  const assets = new Map<string, ValueFlowAsset>()
  for (const item of items) {
    for (const change of item.assetChanges) {
      const {id} = change.asset
      const previous = assets.get(id)
      assets.set(id, {
        id,
        symbol: previous?.symbol ?? change.asset.symbol,
        decimals: previous?.decimals ?? change.asset.decimals,
      })
    }
  }

  return [...assets.values()].sort((left, right) => {
    return (left.symbol ?? left.id).localeCompare(right.symbol ?? right.id)
  })
}

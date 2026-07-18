import type React from "react"
import {
  ContractChip,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableFooter,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableTable,
} from "@acton/ui"

import type {ContractData, ValueFlowAsset, ValueFlowItem} from "../../model/transaction"
import {formatAddress, formatCurrency, formatDecimalAmount} from "../../lib/format"

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
  const assets = collectAssets(items)
  const numericColumnWidth = `${66 / (assets.length + 2)}%`
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
    <DataTable className={className} minWidth={`${34 + assets.length * 10}rem`}>
      <DataTableTable aria-label="Value flow" rowDividers={false}>
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell columnWidth="34%">Account</DataTableHeaderCell>
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
              {assets.map(asset => {
                const assetChange = item.assetChanges.find(change => change.asset.id === asset.id)
                return (
                  <DataTableCell key={asset.id} align="right">
                    {assetChange && (
                      <span
                        className={
                          assetChange.change > 0n
                            ? `${styles.assetValue} ${styles.positive}`
                            : styles.assetValue
                        }
                      >
                        {formatSignedAssetChange(assetChange.change, asset)}
                      </span>
                    )}
                  </DataTableCell>
                )
              })}
              <DataTableCell align="right">{formatCurrency(item.fee)}</DataTableCell>
            </DataTableRow>
          ))}
        </DataTableBody>
        {showTotal && (
          <DataTableFooter>
            <DataTableRow>
              <DataTableCell colSpan={2 + assets.length} />
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

function formatSignedAssetChange(value: bigint, asset: ValueFlowAsset): string {
  const sign = value > 0n ? "+" : value < 0n ? "-" : ""
  const absolute = value < 0n ? -value : value
  const amount =
    asset.decimals === undefined
      ? absolute.toString()
      : formatDecimalAmount(absolute.toString(), asset.decimals)
  return `${sign}${amount}${asset.symbol ? ` ${asset.symbol}` : ""}`
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

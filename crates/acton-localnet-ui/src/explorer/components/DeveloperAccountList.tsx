import type {FC, ReactNode} from "react"

import type {V3AccountState} from "../api/types"
import type {ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"

import {AddressChip} from "./AddressChip"
import styles from "./DeveloperAccountList.module.css"
import {formatNano} from "./utils"

export interface DeveloperAccountListItem {
  readonly address: string
  readonly state?: V3AccountState
}

interface DeveloperAccountListProps {
  readonly accounts: readonly DeveloperAccountListItem[]
  readonly className?: string
  readonly title?: string
  readonly emptyState?: ReactNode
  readonly onAddressClick?: (address: string, event?: ExplorerNavigationClickEvent) => void
}

export const DeveloperAccountListSkeleton: FC<{
  readonly className?: string
  readonly title?: string
  readonly rows?: number
}> = ({className, title, rows = 4}) => (
  <div
    className={`${styles.tableWrap} ${className ?? ""}`}
    aria-label={title ? `Loading ${title}` : "Loading accounts"}
  >
    {title ? <div className={styles.tableTitle}>{title}</div> : null}
    <table className={styles.table}>
      <thead>
        <tr>
          <th>Account</th>
          <th className={styles.statusHeader}>Status</th>
          <th className={styles.typeHeader}>Type</th>
          <th className={styles.balanceHeader}>Balance</th>
        </tr>
      </thead>
      <tbody>
        {Array.from({length: rows}, (_, index) => (
          <tr key={`developer-account-skeleton-${index}`} className={styles.row}>
            <td className={styles.accountCell}>
              <span className={`${styles.skeletonLine} ${styles.skeletonAccount}`} />
            </td>
            <td className={styles.statusCell}>
              <span className={`${styles.skeletonLine} ${styles.skeletonStatus}`} />
            </td>
            <td className={styles.typeCell}>
              <span className={`${styles.skeletonLine} ${styles.skeletonType}`} />
            </td>
            <td className={styles.balanceCell}>
              <span className={`${styles.skeletonLine} ${styles.skeletonBalance}`} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  </div>
)

export const DeveloperAccountList: FC<DeveloperAccountListProps> = ({
  accounts,
  className,
  title,
  emptyState = "No accounts yet",
  onAddressClick,
}) => {
  if (accounts.length === 0) {
    return <div className={`${styles.emptyState} ${className ?? ""}`}>{emptyState}</div>
  }

  return (
    <div className={`${styles.tableWrap} ${className ?? ""}`}>
      {title ? <div className={styles.tableTitle}>{title}</div> : null}
      <table className={styles.table}>
        <thead>
          <tr>
            <th>Account</th>
            <th className={styles.statusHeader}>Status</th>
            <th className={styles.typeHeader}>Type</th>
            <th className={styles.balanceHeader}>Balance</th>
          </tr>
        </thead>
        <tbody>
          {accounts.map(account => {
            const status = getAccountStatus(account.state)
            const type = getAccountType(account.state)
            const balance = formatAccountBalance(account.state)
            const canOpenAccount = onAddressClick !== undefined

            return (
              <tr
                key={account.address}
                className={`${styles.row} ${canOpenAccount ? styles.rowInteractive : ""}`}
                onClick={event => onAddressClick?.(account.address, event)}
                onKeyDown={event => {
                  if (!canOpenAccount) {
                    return
                  }

                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault()
                    onAddressClick(account.address)
                  }
                }}
                tabIndex={canOpenAccount ? 0 : undefined}
                role={canOpenAccount ? "button" : undefined}
                aria-label={canOpenAccount ? `Open account ${account.address}` : undefined}
              >
                <td className={styles.accountCell}>
                  <AccountCell account={account} onAddressClick={onAddressClick} />
                </td>
                <td className={styles.statusCell}>
                  <span className={`${styles.statusBadge} ${styles[status.className]}`}>
                    {status.label}
                  </span>
                </td>
                <td className={styles.typeCell}>
                  <span className={styles.typeValue}>{type}</span>
                </td>
                <td className={styles.balanceCell}>
                  <span className={styles.balanceText}>{balance}</span>
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}

const AccountCell: FC<{
  readonly account: DeveloperAccountListItem
  readonly onAddressClick?: (address: string, event?: ExplorerNavigationClickEvent) => void
}> = ({account, onAddressClick}) => {
  if (!onAddressClick) {
    return <AddressChip address={account.address} />
  }

  return <AddressChip address={account.address} onAddressClick={onAddressClick} />
}

type AccountStatusClass = "statusActive" | "statusFrozen" | "statusUninit" | "statusNonexist"

interface AccountStatusInfo {
  readonly label: string
  readonly className: AccountStatusClass
}

function getAccountStatus(state: V3AccountState | undefined): AccountStatusInfo {
  switch (state?.status?.trim().toLowerCase()) {
    case "active":
      return {label: "Active", className: "statusActive"}
    case "frozen":
      return {label: "Frozen", className: "statusFrozen"}
    case "nonexist":
      return {label: "Nonexist", className: "statusNonexist"}
    case "uninitialized":
    case "uninit":
      return {label: "Uninit", className: "statusUninit"}
    default:
      return {label: "Unknown", className: "statusUninit"}
  }
}

const KNOWN_ACCOUNT_TYPES: readonly [string, string][] = [
  ["jetton_wallet", "Jetton Wallet"],
  ["jetton_master", "Jetton Master"],
  ["nft_collection", "NFT Collection"],
  ["nft_item", "NFT Item"],
]

function getAccountType(state: V3AccountState | undefined): string {
  const interfaces = Array.isArray(state?.interfaces)
    ? state.interfaces.map(iface => iface.trim().toLowerCase())
    : []
  for (const [name, label] of KNOWN_ACCOUNT_TYPES) {
    if (interfaces.includes(name)) {
      return label
    }
  }

  if (interfaces.some(iface => iface.includes("wallet"))) {
    return "Wallet"
  }

  if (state?.code_hash) {
    return "Contract"
  }

  return "Unknown"
}

function formatAccountBalance(state: V3AccountState | undefined): string {
  if (!state?.balance) {
    return "—"
  }

  return `${formatNano(state.balance)} GRAM`
}

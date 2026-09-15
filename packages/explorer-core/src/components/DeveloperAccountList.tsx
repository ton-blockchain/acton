import {
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  GramAmount,
} from "@acton/ui"
import type {FC, ReactNode} from "react"

import type {V3AccountState} from "../api/types"
import type {ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"

import {ExplorerAddressChip} from "./ExplorerAddressChip"
import styles from "./DeveloperAccountList.module.css"

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
  <DataTable
    className={className}
    minWidth="42.5rem"
    title={title}
    aria-label={title ? `Loading ${title}` : "Loading accounts"}
    aria-busy
  >
    <DataTableTable aria-label={title ?? "Accounts"}>
      <AccountTableHead />
      <DataTableBody>
        <DataTableSkeletonRows
          alignments={["left", "left", "left", "right"]}
          columns={4}
          rowKeyPrefix="developer-account-skeleton"
          rows={rows}
          widths={["76%", "62px", "78%", "92px"]}
        />
      </DataTableBody>
    </DataTableTable>
  </DataTable>
)

export const DeveloperAccountList: FC<DeveloperAccountListProps> = ({
  accounts,
  className,
  title,
  emptyState = "No accounts yet",
  onAddressClick,
}) => {
  return (
    <DataTable className={className} minWidth="42.5rem" title={title}>
      <DataTableTable aria-label={title ?? "Accounts"}>
        <AccountTableHead />
        <DataTableBody>
          {accounts.length === 0 ? (
            <DataTableEmpty colSpan={4}>{emptyState}</DataTableEmpty>
          ) : (
            accounts.map(account => {
              const status = getAccountStatus(account.state)
              const type = getAccountType(account.state)
              const balance = formatAccountBalance(account.state)
              const canOpenAccount = onAddressClick !== undefined

              return (
                <DataTableRow
                  key={account.address}
                  interactive={canOpenAccount}
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
                  <DataTableCell truncate>
                    <ExplorerAddressChip
                      address={account.address}
                      onAddressClick={onAddressClick}
                    />
                  </DataTableCell>
                  <DataTableCell>
                    <span className={`${styles.statusBadge} ${styles[status.className]}`}>
                      {status.label}
                    </span>
                  </DataTableCell>
                  <DataTableCell tone="muted" truncate>
                    {type}
                  </DataTableCell>
                  <DataTableCell align="right" tone="strong">
                    {balance}
                  </DataTableCell>
                </DataTableRow>
              )
            })
          )}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

function AccountTableHead() {
  return (
    <DataTableHead>
      <DataTableRow>
        <DataTableHeaderCell>Account</DataTableHeaderCell>
        <DataTableHeaderCell columnWidth="98px">Status</DataTableHeaderCell>
        <DataTableHeaderCell columnWidth="132px">Type</DataTableHeaderCell>
        <DataTableHeaderCell align="right" columnWidth="260px">
          Balance
        </DataTableHeaderCell>
      </DataTableRow>
    </DataTableHead>
  )
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
  ["jetton_wallet", "Jetton wallet"],
  ["jetton_master", "Jetton master"],
  ["nft_collection", "NFT collection"],
  ["nft_item", "NFT item"],
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

function formatAccountBalance(state: V3AccountState | undefined): ReactNode {
  if (!state?.balance) {
    return "—"
  }

  return <GramAmount value={state.balance} useGrouping />
}

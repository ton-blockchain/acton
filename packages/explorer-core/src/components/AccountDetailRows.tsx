import type {FC, ReactNode} from "react"

import {ExplorerAddressChip} from "./ExplorerAddressChip"

import styles from "./AccountDetailRows.module.css"

interface AccountDetailRowsProps {
  readonly children: ReactNode
}

export const AccountDetailRows: FC<AccountDetailRowsProps> = ({children}) => (
  <div className={styles.rows}>{children}</div>
)

interface AccountTextDetailRowProps {
  readonly label: string
  readonly value: ReactNode
}

export const AccountTextDetailRow: FC<AccountTextDetailRowProps> = ({label, value}) => (
  <div className={styles.row}>
    <span className={styles.label}>{label}</span>
    <span className={styles.value}>{value}</span>
  </div>
)

interface AccountAddressDetailRowProps {
  readonly label: string
  readonly address?: string
  readonly fallback?: string
  readonly onAddressClick: (address: string) => void
}

export const AccountAddressDetailRow: FC<AccountAddressDetailRowProps> = ({
  label,
  address,
  fallback,
  onAddressClick,
}) => (
  <div className={styles.row}>
    <span className={styles.label}>{label}</span>
    {address === undefined ? (
      <span className={styles.value}>{fallback ?? "Unknown"}</span>
    ) : (
      <div className={styles.addressValue}>
        <ExplorerAddressChip address={address} onAddressClick={onAddressClick} variant="plain" />
      </div>
    )}
  </div>
)

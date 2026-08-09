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
  formatOpcode,
  GramAmount,
  RelativeTime,
} from "@acton/ui"
import type {FC, ReactNode} from "react"

import {addressKey} from "../api/compilerAbi"
import type {V3Message, V3TransactionListItem} from "../api/types"
import type {ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"

import {ExplorerAddressChip} from "./ExplorerAddressChip"
import {hashToHex} from "./utils"
import type {MessageNamesByAddress} from "../hooks/useMessageNamesByAddress"

import styles from "./DeveloperTransactionList.module.css"

export type TransactionListItem = V3TransactionListItem

type TransactionMessage = V3Message

type DeveloperEndpoint =
  | {
      readonly kind: "address"
      readonly address: string
      readonly fallback: string
    }
  | {readonly kind: "text"; readonly label: string; readonly title?: string}

interface DeveloperTransactionRow {
  readonly key: string
  readonly transaction: TransactionListItem
  readonly time: number
  readonly from: DeveloperEndpoint
  readonly to: DeveloperEndpoint
  readonly direction: "IN" | "OUT"
  readonly messageName?: string
  readonly valueLabel: string
  readonly valueNanograms?: bigint
  readonly valueKind: "value" | "empty"
  readonly isSuccess: boolean
  readonly statusLabel: string
}

interface DeveloperTransactionListProps {
  readonly transactions: readonly TransactionListItem[]
  readonly className?: string
  readonly title?: string
  readonly emptyState?: ReactNode
  readonly maxRows?: number
  readonly messageNamesByAddress?: MessageNamesByAddress
  readonly onTransactionClick?: (
    hashHex: string,
    transaction: TransactionListItem,
    event?: ExplorerNavigationClickEvent,
  ) => void
  readonly onAddressClick?: (address: string, event?: ExplorerNavigationClickEvent) => void
}

export const DeveloperTransactionListSkeleton: FC<{
  readonly className?: string
  readonly title?: string
  readonly rows?: number
}> = ({className, title, rows = 5}) => (
  <DataTable
    className={className}
    title={title}
    minWidth="45rem"
    aria-label={title ? `Loading ${title}` : "Loading transactions"}
  >
    <DataTableTable aria-busy="true" aria-label={title ?? "Loading transactions"} layout="fixed">
      <DataTableHead>
        <DataTableRow>
          <DataTableHeaderCell className={styles.timeCell} columnWidth="6.25rem">
            Time
          </DataTableHeaderCell>
          <DataTableHeaderCell align="right">From</DataTableHeaderCell>
          <DataTableHeaderCell
            className={styles.directionCell}
            columnWidth="3.125rem"
            aria-label="Direction"
          />
          <DataTableHeaderCell>To</DataTableHeaderCell>
          <DataTableHeaderCell columnWidth="15rem">Opcode</DataTableHeaderCell>
          <DataTableHeaderCell align="right" columnWidth="12rem">
            Value
          </DataTableHeaderCell>
        </DataTableRow>
      </DataTableHead>
      <DataTableBody>
        <DataTableSkeletonRows
          columns={6}
          rows={rows}
          alignments={["left", "right", "center", "left", "left", "right"]}
          widths={["3.5rem", "10rem", "1rem", "10rem", "10rem", "5rem"]}
        />
      </DataTableBody>
    </DataTableTable>
  </DataTable>
)

export const DeveloperTransactionList: FC<DeveloperTransactionListProps> = ({
  transactions,
  className,
  title,
  emptyState = "No transactions yet",
  maxRows,
  messageNamesByAddress,
  onTransactionClick,
  onAddressClick,
}) => {
  const allRows = transactions.flatMap(transaction =>
    buildDeveloperRows(transaction, messageNamesByAddress),
  )
  const rows = maxRows === undefined ? allRows : allRows.slice(0, maxRows)

  return (
    <DataTable className={className} title={title} minWidth="45rem" aria-label={title}>
      <DataTableTable aria-label={title ?? "Transactions"} layout="fixed">
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell className={styles.timeCell} columnWidth="6.25rem">
              Time
            </DataTableHeaderCell>
            <DataTableHeaderCell align="right">From</DataTableHeaderCell>
            <DataTableHeaderCell
              className={styles.directionCell}
              columnWidth="3.125rem"
              aria-label="Direction"
            />
            <DataTableHeaderCell>To</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="15rem">Opcode</DataTableHeaderCell>
            <DataTableHeaderCell align="right" columnWidth="12rem">
              Value
            </DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {rows.length === 0 ? (
            <DataTableEmpty colSpan={6}>{emptyState}</DataTableEmpty>
          ) : (
            rows.map(row => {
              const hashHex = hashToHex(getTransactionHash(row.transaction))
              const canOpenTransaction = hashHex !== undefined && onTransactionClick !== undefined
              return (
                <DataTableRow
                  key={row.key}
                  interactive={canOpenTransaction}
                  tabIndex={canOpenTransaction ? 0 : undefined}
                  onClick={event => {
                    if (hashHex) {
                      onTransactionClick?.(hashHex, row.transaction, event)
                    }
                  }}
                  onKeyDown={event => {
                    if (
                      hashHex &&
                      (event.key === "Enter" || event.key === " ") &&
                      onTransactionClick
                    ) {
                      event.preventDefault()
                      onTransactionClick(hashHex, row.transaction)
                    }
                  }}
                  title={row.statusLabel}
                >
                  <DataTableCell className={styles.timeCell} tone="muted">
                    <RelativeTime value={row.time} unit="seconds" mode="hybrid" />
                  </DataTableCell>
                  <DataTableCell align="right">
                    <EndpointCell
                      endpoint={row.from}
                      copyPlacement="left"
                      onAddressClick={onAddressClick}
                    />
                  </DataTableCell>
                  <DataTableCell className={styles.directionCell} align="center">
                    <span
                      className={`${styles.directionBadge} ${
                        row.direction === "IN" ? styles.directionIn : styles.directionOut
                      }`}
                    >
                      {row.direction}
                    </span>
                  </DataTableCell>
                  <DataTableCell>
                    <EndpointCell endpoint={row.to} onAddressClick={onAddressClick} />
                  </DataTableCell>
                  <DataTableCell tone="muted">
                    <span className={styles.opcodeValue}>{row.messageName ?? "—"}</span>
                  </DataTableCell>
                  <DataTableCell align="right" tone="strong">
                    <span
                      className={`${styles.valueText} ${
                        row.valueKind === "empty" ? styles.valueEmpty : ""
                      }`}
                    >
                      {row.valueNanograms === undefined ? (
                        row.valueLabel
                      ) : (
                        <GramAmount value={row.valueNanograms} useGrouping />
                      )}
                    </span>
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

const EndpointCell: FC<{
  readonly endpoint: DeveloperEndpoint
  readonly copyPlacement?: "left" | "right"
  readonly onAddressClick?: (address: string, event?: ExplorerNavigationClickEvent) => void
}> = ({endpoint, copyPlacement = "right", onAddressClick}) => {
  if (endpoint.kind === "text") {
    return (
      <span className={styles.endpointText} title={endpoint.title}>
        {endpoint.label}
      </span>
    )
  }

  if (!onAddressClick) {
    return (
      <ExplorerAddressChip
        address={endpoint.address}
        fallback={endpoint.fallback}
        copyPlacement={copyPlacement}
      />
    )
  }

  return (
    <ExplorerAddressChip
      address={endpoint.address}
      fallback={endpoint.fallback}
      copyPlacement={copyPlacement}
      onAddressClick={onAddressClick}
    />
  )
}

function buildDeveloperRows(
  transaction: TransactionListItem,
  messageNamesByAddress?: MessageNamesByAddress,
): DeveloperTransactionRow[] {
  const rows: DeveloperTransactionRow[] = []
  const time = getTransactionTime(transaction)
  const account = transaction.account
  const isSuccess = isTransactionSuccess(transaction)
  const statusLabel = getTransactionStatusLabel(transaction)
  const transactionHash = getTransactionHash(transaction)
  const transactionKey = transactionHash

  transaction.out_msgs.forEach((message, index) => {
    const to = addressEndpoint(message.destination, "External")
    const value = formatMessageValue(message, to)
    rows.push({
      key: `${transactionKey}:out:${message.hash || index}`,
      transaction,
      time,
      from: addressEndpoint(message.source || account, "Account"),
      to,
      direction: "OUT",
      messageName: resolveMessageLabel(message, messageNamesByAddress),
      valueLabel: value.label,
      valueNanograms: value.nanograms,
      valueKind: value.kind,
      isSuccess,
      statusLabel,
    })
  })

  if (transaction.in_msg) {
    const from = addressEndpoint(transaction.in_msg.source, "External")
    const value = formatMessageValue(transaction.in_msg, from)
    rows.push({
      key: `${transactionKey}:in`,
      transaction,
      time,
      from,
      to: addressEndpoint(transaction.in_msg.destination || account, "Account"),
      direction: "IN",
      messageName: resolveMessageLabel(transaction.in_msg, messageNamesByAddress),
      valueLabel: value.label,
      valueNanograms: value.nanograms,
      valueKind: value.kind,
      isSuccess,
      statusLabel,
    })
  }

  if (rows.length === 0) {
    rows.push({
      key: `${transactionKey}:empty`,
      transaction,
      time,
      from: textEndpoint("System"),
      to: addressEndpoint(account, "Account"),
      direction: "IN",
      valueLabel: "empty",
      valueKind: "empty",
      isSuccess,
      statusLabel,
    })
  }

  return rows
}

function getTransactionTime(transaction: TransactionListItem): number {
  return transaction.now
}

function getTransactionHash(transaction: TransactionListItem): string {
  return transaction.hash
}

function isTransactionSuccess(transaction: TransactionListItem): boolean {
  return (
    !transaction.description.aborted &&
    transaction.description.compute_ph.success &&
    transaction.description.action.success
  )
}

function getTransactionStatusLabel(transaction: TransactionListItem): string {
  if (isTransactionSuccess(transaction)) {
    return "Confirmed transaction"
  }

  return `Failed transaction, exit ${transaction.description.compute_ph.exit_code}`
}

function addressEndpoint(address: string | undefined, fallback: string): DeveloperEndpoint {
  return address ? {kind: "address", address, fallback} : textEndpoint(fallback)
}

function textEndpoint(label: string, title?: string): DeveloperEndpoint {
  return {kind: "text", label, title}
}

function parseNanoValue(value: string | number | undefined): bigint {
  if (value === undefined) {
    return 0n
  }

  try {
    return BigInt(value)
  } catch {
    return 0n
  }
}

function formatMessageValue(
  message: TransactionMessage,
  externalEndpoint: DeveloperEndpoint,
): {label: string; nanograms?: bigint; kind: "value" | "empty"} {
  if (externalEndpoint.kind === "text" && externalEndpoint.label === "External") {
    return {label: "empty", kind: "empty"}
  }

  const value = parseNanoValue(message.value)
  if (value === 0n) {
    return {label: "empty", kind: "empty"}
  }

  return {label: "", nanograms: value, kind: "value"}
}

function formatMessageOpcode(message: TransactionMessage | undefined): string | undefined {
  if (!message || !("opcode" in message)) {
    return undefined
  }

  return formatOpcode(message.opcode)
}

function resolveMessageName(
  message: TransactionMessage | undefined,
  messageNamesByAddress?: MessageNamesByAddress,
): string | undefined {
  if (!message || !messageNamesByAddress) {
    return undefined
  }

  const opcode = formatMessageOpcode(message)
  if (!opcode) {
    return undefined
  }

  const destinationNames = message.destination
    ? messageNamesByAddress.get(addressKey(message.destination))
    : undefined
  const sourceNames = message.source
    ? messageNamesByAddress.get(addressKey(message.source))
    : undefined

  return destinationNames?.incoming.get(opcode) ?? sourceNames?.outgoing.get(opcode) ?? undefined
}

function resolveMessageLabel(
  message: TransactionMessage | undefined,
  messageNamesByAddress?: MessageNamesByAddress,
): string | undefined {
  return resolveMessageName(message, messageNamesByAddress) ?? formatMessageOpcode(message)
}

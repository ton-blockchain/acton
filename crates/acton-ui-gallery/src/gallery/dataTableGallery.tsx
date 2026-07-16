import {
  Button,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableFooter,
  DataTableGroupRow,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  InlineButton,
  Input,
} from "@acton/ui"
import {ExternalLink, Link2, RefreshCw, Unplug} from "lucide-react"
import {useEffect, useRef, useState} from "react"

import styles from "./dataTableGallery.module.css"
import type {ComponentGallery} from "./types"

const walletRows = [
  {
    name: "deployer",
    address: "EQD36Xy6k6sDYVfDfYwYpQQ0YB7lyd7R5s6XW4ur8XSS",
    version: "V5R1",
    balance: "100.2519 GRAM",
  },
  {
    name: "treasury",
    address: "EQB8YtZZA7Kzz3cH8B36fb5FTB2v3Gj1lMaqYGFN4q8",
    version: "V5R1",
    balance: "42.0000 GRAM",
  },
] as const

const sessionRows = [
  {
    dapp: "TON Unfreezer",
    domain: "unfreezer.ton.org",
    wallet: "deployer",
    activity: "6/20/2026, 5:14:31 AM",
  },
] as const

const traceFeeRows = [
  {
    trace: "Trace 1",
    message: "empty",
    txCount: "1",
    gasUsed: "309",
    gasFee: "0.000020601 GRAM",
    forwardFee: "0.000089869 GRAM",
    totalFee: "0.000020601 GRAM",
    treasury: true,
  },
  {
    trace: "Trace 2",
    message: "empty",
    txCount: "1",
    gasUsed: "309",
    gasFee: "0.000020601 GRAM",
    forwardFee: "0.000090935 GRAM",
    totalFee: "0.000020601 GRAM",
    treasury: true,
  },
  {
    trace: "Trace 3",
    message: "TopUpTons",
    txCount: "1",
    gasUsed: "1189",
    gasFee: "0.000079268 GRAM",
    forwardFee: "0.00073374 GRAM",
    totalFee: "0.000079268 GRAM",
    treasury: false,
  },
  {
    trace: "Trace 4",
    message: "DropMinterAdmin",
    txCount: "1",
    gasUsed: "1967",
    gasFee: "0.000131134 GRAM",
    forwardFee: "0.000044446 GRAM",
    totalFee: "0.000131134 GRAM",
    treasury: false,
  },
] as const

function StartupWalletsSample() {
  const [isRefreshing, setIsRefreshing] = useState(false)
  const refreshTimeoutRef = useRef<ReturnType<typeof globalThis.setTimeout> | undefined>(undefined)

  useEffect(() => {
    return () => {
      if (refreshTimeoutRef.current !== undefined) {
        globalThis.clearTimeout(refreshTimeoutRef.current)
      }
    }
  }, [])

  const handleRefresh = () => {
    if (refreshTimeoutRef.current !== undefined) {
      globalThis.clearTimeout(refreshTimeoutRef.current)
    }

    setIsRefreshing(true)
    refreshTimeoutRef.current = globalThis.setTimeout(() => {
      refreshTimeoutRef.current = undefined
      setIsRefreshing(false)
    }, 3000)
  }

  return (
    <DataTable
      title="Startup wallets"
      minWidth="54rem"
      actions={
        <InlineButton
          variant="accent"
          leadingIcon={
            <RefreshCw className={isRefreshing ? styles.spinningIcon : undefined} size={15} />
          }
          aria-busy={isRefreshing || undefined}
          aria-label={isRefreshing ? "Refreshing startup wallets" : "Refresh startup wallets"}
          disabled={isRefreshing}
          onClick={handleRefresh}
        >
          {isRefreshing ? "Refreshing" : "Refresh"}
        </InlineButton>
      }
    >
      <DataTableTable aria-label="Startup wallets">
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell columnWidth="10rem">Name</DataTableHeaderCell>
            <DataTableHeaderCell>Address</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="8rem">Version</DataTableHeaderCell>
            <DataTableHeaderCell align="right" columnWidth="14rem">
              Balance
            </DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {walletRows.map(wallet => (
            <DataTableRow key={wallet.address} hover>
              <DataTableCell tone="strong" truncate>
                {wallet.name}
              </DataTableCell>
              <DataTableCell truncate>
                <span className={styles.linkValue}>{compactMiddle(wallet.address)}</span>
              </DataTableCell>
              <DataTableCell tone="strong">{wallet.version}</DataTableCell>
              <DataTableCell align="right" tone="strong">
                {wallet.balance}
              </DataTableCell>
            </DataTableRow>
          ))}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

function SessionsSample() {
  return (
    <DataTable title="Sessions" meta="No pending approvals" minWidth="62rem">
      <DataTableTable aria-label="TON Connect sessions">
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell columnWidth="22rem">Dapp</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="14rem">Wallet</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="15rem">Last activity</DataTableHeaderCell>
            <DataTableHeaderCell align="right" columnWidth="10rem" aria-label="Actions" />
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {sessionRows.map(session => (
            <DataTableRow key={session.domain} hover>
              <DataTableCell truncate>
                <span className={styles.sessionLine}>
                  <span className={styles.sessionTitle}>{session.dapp}</span>
                  <span className={styles.sessionSeparator}>·</span>
                  <span className={styles.sessionDomain}>{session.domain}</span>
                </span>
              </DataTableCell>
              <DataTableCell truncate>
                <span className={styles.linkValue}>{session.wallet}</span>
              </DataTableCell>
              <DataTableCell tone="muted" truncate>
                {session.activity}
              </DataTableCell>
              <DataTableCell align="right">
                <InlineButton
                  variant="accent"
                  leadingIcon={<Unplug size={15} />}
                  aria-label={`Disconnect ${session.dapp}`}
                >
                  Disconnect
                </InlineButton>
              </DataTableCell>
            </DataTableRow>
          ))}
        </DataTableBody>
        <DataTableFooter>
          <DataTableRow>
            <DataTableCell colSpan={4} className={styles.connectCell}>
              <form className={styles.connectForm}>
                <label className={styles.connectLabel} htmlFor="gallery-ton-connect-url">
                  Connect URL
                </label>
                <Input
                  size="sm"
                  id="gallery-ton-connect-url"
                  placeholder="tonconnect://..."
                  aria-label="TON Connect URL"
                />
                <Button
                  variant="outline"
                  size="sm"
                  className={styles.connectAction}
                  leadingIcon={<Link2 size={14} />}
                  type="button"
                  disabled
                >
                  Handle request
                </Button>
              </form>
            </DataTableCell>
          </DataTableRow>
        </DataTableFooter>
      </DataTableTable>
    </DataTable>
  )
}

function LoadingWalletsSample() {
  return (
    <DataTable title="Loading wallets" minWidth="54rem" aria-label="Loading wallet table">
      <DataTableTable>
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell columnWidth="10rem">Name</DataTableHeaderCell>
            <DataTableHeaderCell>Address</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="8rem">Version</DataTableHeaderCell>
            <DataTableHeaderCell align="right" columnWidth="14rem">
              Balance
            </DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          <DataTableSkeletonRows
            rows={4}
            columns={4}
            widths={["7rem", "18rem", "4rem", "7rem"]}
            alignments={["left", "left", "left", "right"]}
          />
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

function WalletRuntimeSample() {
  return (
    <div className={styles.runtimeStack}>
      <StartupWalletsSample />
      <SessionsSample />
    </div>
  )
}

function CollapsibleRowsSample() {
  const [expanded, setExpanded] = useState(false)
  const treasuryRows = traceFeeRows.filter(row => row.treasury)
  const visibleRows = traceFeeRows.filter(row => !row.treasury)

  return (
    <DataTable minWidth="62rem" aria-label="Trace fee summary">
      <DataTableTable>
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell columnWidth="16rem">Trace</DataTableHeaderCell>
            <DataTableHeaderCell align="center" columnWidth="8rem">
              Tx Count
            </DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="8rem">Gas Used</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="12rem">Gas Fee</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="13rem">Forward Fee</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="13rem">Total Fee</DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          <DataTableGroupRow
            colSpan={6}
            expanded={expanded}
            onToggle={() => setExpanded(current => !current)}
          >
            {treasuryRows.length} treasury deploys
          </DataTableGroupRow>
          {expanded &&
            treasuryRows.map(row => <TraceFeeRow key={row.trace} row={row} groupChild />)}
          {visibleRows.map(row => (
            <TraceFeeRow key={row.trace} row={row} />
          ))}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

function TraceFeeRow({
  groupChild = false,
  row,
}: {
  readonly groupChild?: boolean
  readonly row: (typeof traceFeeRows)[number]
}) {
  return (
    <DataTableRow hover groupChild={groupChild}>
      <DataTableCell truncate>
        <button type="button" className={styles.traceLink}>
          <span>{row.trace}</span>
          <span className={styles.traceSeparator}>·</span>
          <span className={styles.traceMessage}>{row.message}</span>
          <ExternalLink size={13} aria-hidden="true" />
        </button>
      </DataTableCell>
      <DataTableCell align="center" tone="strong">
        {row.txCount}
      </DataTableCell>
      <DataTableCell tone="strong">{row.gasUsed}</DataTableCell>
      <DataTableCell tone="strong">{row.gasFee}</DataTableCell>
      <DataTableCell tone="strong">{row.forwardFee}</DataTableCell>
      <DataTableCell tone="strong">{row.totalFee}</DataTableCell>
    </DataTableRow>
  )
}

function compactMiddle(value: string) {
  if (value.length <= 18) return value
  return `${value.slice(0, 6)}...${value.slice(-6)}`
}

export const dataTableGallery = {
  id: "data-table",
  title: "DataTable",
  status: "ready",
  summary:
    "DataTable renders standalone framed tables with optional title actions, dense rows, loading rows, empty states, and collapsible row groups.",
  importStatement: 'import { DataTable, DataTableTable, DataTableRow } from "@acton/ui"',
  agentSummary:
    "Use DataTable as the shared shell for localnet/explorer/test tables. Keep domain rendering inside cells, and use DataTableGroupRow for collapsible row sections.",
  usage: [
    "Use for standalone data sets that need a bordered frame, optional title bar, column headers, and dense rows.",
    "Use DataTableGroupRow when a group label expands or collapses related rows inside the same tbody.",
    "Use DataTableSkeletonRows for repeated row loading states instead of local shimmer CSS.",
  ],
  avoid: [
    "Do not put domain formatting or routing logic inside DataTable.",
    "Do not replace semantic table elements with grid markup for tabular data.",
    "Do not duplicate table frame/header/row CSS in feature code when this structure fits.",
  ],
  sections: [
    {
      id: "data-table-wallet-runtime",
      title: "Wallet Runtime",
      description:
        "Standalone localnet tables with title action, metadata, row actions, and footer controls.",
      content: <WalletRuntimeSample />,
    },
    {
      id: "data-table-collapsible",
      title: "Collapsible Rows",
      description: "A group row controls visibility for related child rows inside the same table.",
      content: <CollapsibleRowsSample />,
    },
    {
      id: "data-table-loading",
      title: "Loading Rows",
      description:
        "Shared skeleton rows keep column widths and the table frame stable while data loads.",
      content: <LoadingWalletsSample />,
    },
  ],
} satisfies ComponentGallery

import {
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableFooter,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
} from "@acton/ui"
import {useEffect, useMemo, useState, type FC} from "react"

import type {TonClient} from "../api/client"
import type {SuspendedAccountsConfig} from "../api/suspendedAccounts"
import {ExplorerAddressChip} from "../components/ExplorerAddressChip"
import {ExplorerBreadcrumbs} from "../components/ExplorerBreadcrumbs"
import {formatNano, toRawAddress} from "../components/utils"
import {useOpenExplorerPath} from "../hooks/useOpenExplorerPath"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useNetworkInfo} from "../hooks/useNetworkInfo"
import styles from "./SuspendedAddressesPage.module.css"

interface SuspendedAddressesPageProps {
  readonly client: TonClient
}

interface SuspendedAccountRow {
  readonly rawAddress: string
  readonly balance?: string
}

type SuspendedAccountsLoadState =
  | {readonly status: "loading"}
  | {
      readonly status: "success"
      readonly config: SuspendedAccountsConfig
      readonly balances: ReadonlyMap<string, string>
      readonly balancesStatus: "error" | "loading" | "success"
    }
  | {readonly status: "error"; readonly message: string}

const ACCOUNT_STATE_BATCH_SIZE = 50
const SUSPENSION_STATUS_REFRESH_MS = 60_000
const SUSPENDED_ACCOUNTS_VOTE_URL = "https://t.me/tonblockchain/182"
const UNLOCK_DATE_FORMATTER = new Intl.DateTimeFormat("en-US", {
  day: "numeric",
  month: "long",
  year: "numeric",
  timeZone: "UTC",
})

export const SuspendedAddressesPage: FC<SuspendedAddressesPageProps> = ({client}) => {
  const routes = useExplorerRoutePaths()
  const openPath = useOpenExplorerPath()
  const {isMainnetFork} = useNetworkInfo()
  const [loadState, setLoadState] = useState<SuspendedAccountsLoadState>({status: "loading"})
  const [nowSeconds, setNowSeconds] = useState(() => Math.floor(Date.now() / 1000))

  useEffect(() => {
    const interval = globalThis.setInterval(
      () => setNowSeconds(Math.floor(Date.now() / 1000)),
      SUSPENSION_STATUS_REFRESH_MS,
    )
    return () => globalThis.clearInterval(interval)
  }, [])

  useEffect(() => {
    let active = true

    const load = async () => {
      setLoadState({status: "loading"})
      try {
        const config = await client.getSuspendedAccountsConfig()
        if (!active) return

        const suspensionActive = config.suspendedUntil > Math.floor(Date.now() / 1000)
        setLoadState({
          status: "success",
          config,
          balances: new Map(),
          balancesStatus: suspensionActive ? "loading" : "success",
        })
        if (!suspensionActive) return

        const batches = chunk(config.rawAddresses, ACCOUNT_STATE_BATCH_SIZE)
        const responses = await Promise.allSettled(
          batches.map(addresses => client.getAccountStates(addresses, false)),
        )
        if (!active) return

        const balances = new Map<string, string>()
        let balancesStatus: "error" | "success" = "success"
        for (const response of responses) {
          if (response.status === "rejected") {
            balancesStatus = "error"
            console.error("Failed to fetch suspended account balances", response.reason)
            continue
          }
          for (const account of response.value.accounts) {
            balances.set(toRawAddress(account.address), account.balance)
          }
        }

        setLoadState({status: "success", config, balances, balancesStatus})
      } catch (error) {
        if (active) {
          setLoadState({
            status: "error",
            message: error instanceof Error ? error.message : String(error),
          })
        }
      }
    }

    void load()
    return () => {
      active = false
    }
  }, [client])

  const config = loadState.status === "success" ? loadState.config : undefined
  const suspensionActive = config ? config.suspendedUntil > nowSeconds : false
  const rows = useMemo<readonly SuspendedAccountRow[]>(() => {
    if (loadState.status !== "success" || !suspensionActive) return []

    return loadState.config.rawAddresses
      .map(rawAddress => ({
        rawAddress,
        balance: loadState.balances.get(rawAddress),
      }))
      .sort(compareSuspendedAccounts)
  }, [loadState, suspensionActive])
  const totalBalance = rows.reduce((total, row) => total + BigInt(row.balance ?? 0), 0n)

  return (
    <section className={styles.container}>
      <ExplorerBreadcrumbs items={[{label: "Suspended addresses"}]} />

      <header className={styles.header}>
        <div className={styles.heading}>
          <h1 className={styles.title}>Suspended addresses</h1>
          <p className={styles.description}>
            {config && suspensionActive ? (
              <>
                {config.rawAddresses.length} addresses are suspended through{" "}
                {isMainnetFork ? (
                  <a
                    className={styles.descriptionLink}
                    href={SUSPENDED_ACCOUNTS_VOTE_URL}
                    target="_blank"
                    rel="noreferrer"
                  >
                    validators&apos; voting
                  </a>
                ) : (
                  "validators' voting"
                )}{" "}
                until {formatUnlockDate(config.suspendedUntil)}
              </>
            ) : config ? (
              "No addresses are currently suspended by the network configuration"
            ) : (
              "Addresses temporarily restricted by the current network configuration"
            )}
          </p>
        </div>
      </header>

      {loadState.status === "error" ? (
        <section className={styles.error} role="alert">
          <h2>Suspended addresses are unavailable</h2>
          <p>{loadState.message}</p>
        </section>
      ) : (
        <DataTable minWidth="48rem">
          <DataTableTable
            aria-busy={
              loadState.status === "loading" ||
              (loadState.status === "success" && loadState.balancesStatus === "loading")
            }
            aria-label="Suspended addresses"
          >
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell columnWidth="3.75rem">#</DataTableHeaderCell>
                <DataTableHeaderCell>Address</DataTableHeaderCell>
                <DataTableHeaderCell align="right" columnWidth="18rem">
                  Balance
                </DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {loadState.status === "loading" ? (
                <DataTableSkeletonRows
                  columns={3}
                  rows={10}
                  alignments={["left", "left", "right"]}
                  widths={["2rem", "30rem", "12rem"]}
                />
              ) : rows.length === 0 ? (
                <DataTableEmpty colSpan={3}>No suspended addresses</DataTableEmpty>
              ) : (
                rows.map((row, index) => {
                  return (
                    <DataTableRow key={row.rawAddress} hover>
                      <DataTableCell tone="muted">{index + 1}</DataTableCell>
                      <DataTableCell truncate>
                        <ExplorerAddressChip
                          address={row.rawAddress}
                          onAddressClick={(address, event) =>
                            openPath(routes.addressPath(address), event)
                          }
                          resolveName={false}
                          shorten={false}
                        />
                      </DataTableCell>
                      <DataTableCell align="right" tone="strong">
                        {row.balance === undefined
                          ? loadState.balancesStatus === "loading"
                            ? "Loading…"
                            : "—"
                          : `${formatNano(row.balance)} GRAM`}
                      </DataTableCell>
                    </DataTableRow>
                  )
                })
              )}
            </DataTableBody>
            {loadState.status === "success" && suspensionActive && (
              <DataTableFooter>
                <DataTableRow>
                  <DataTableCell
                    className={styles.totalCell}
                    colSpan={2}
                    role="rowheader"
                    tone="strong"
                  >
                    Total balance
                  </DataTableCell>
                  <DataTableCell className={styles.totalCell} align="right" tone="strong">
                    {loadState.balancesStatus === "loading"
                      ? "Loading…"
                      : loadState.balancesStatus === "error"
                        ? "Unavailable"
                        : `${formatNano(totalBalance.toString())} GRAM`}
                  </DataTableCell>
                </DataTableRow>
              </DataTableFooter>
            )}
          </DataTableTable>
        </DataTable>
      )}
    </section>
  )
}

function compareSuspendedAccounts(left: SuspendedAccountRow, right: SuspendedAccountRow): number {
  const leftBalance = BigInt(left.balance ?? 0)
  const rightBalance = BigInt(right.balance ?? 0)
  if (leftBalance === rightBalance) {
    return left.rawAddress.localeCompare(right.rawAddress)
  }
  return leftBalance > rightBalance ? -1 : 1
}

function formatUnlockDate(timestamp: number): string {
  return UNLOCK_DATE_FORMATTER.format(new Date(timestamp * 1000))
}

function chunk<T>(items: readonly T[], size: number): T[][] {
  const chunks: T[][] = []
  for (let index = 0; index < items.length; index += size) {
    chunks.push(items.slice(index, index + size))
  }
  return chunks
}

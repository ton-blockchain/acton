import {BookOpen, Check, Copy} from "lucide-react"
import {Card, CardContent, CardHeader, CardTitle, useToast} from "@acton/shared-ui"
import {Link, useNavigate} from "react-router-dom"
import {useCallback, useEffect, useMemo, useState} from "react"
import type {FC} from "react"

import type {TonClient} from "../../explorer/api/client"
import {addressKey} from "../../explorer/api/compilerAbi"
import type {
  LocalnetNodeInfo,
  V3AccountState,
  V3TransactionListItem,
} from "../../explorer/api/types"
import {
  DeveloperAccountList,
  DeveloperAccountListSkeleton,
  type DeveloperAccountListItem,
} from "../../explorer/components/DeveloperAccountList"
import {
  DeveloperTransactionList,
  DeveloperTransactionListSkeleton,
} from "../../explorer/components/DeveloperTransactionList"
import {formatDuration} from "../../explorer/components/utils"
import {useAddressBook} from "../../explorer/hooks/useAddressBook"
import {useOpenExplorerPath} from "../../explorer/hooks/useOpenExplorerPath"
import {useTransactionMessageNames} from "../../explorer/hooks/useTransactionMessageNames"
import {collectRecentAccounts} from "../dashboardUtils"

import styles from "../DashboardPage.module.css"

const HOME_RECENT_TRANSACTIONS_REFRESH_MS = 2000
const HOME_NODE_INFO_REFRESH_MS = 1000
const MASTERCHAIN_BLOCK_SHARD = "8000000000000000"

interface HomePageProps {
  readonly client: TonClient
}

interface HomeState {
  readonly transactions: readonly V3TransactionListItem[]
  readonly accountStatesByAddress: Readonly<Record<string, V3AccountState>>
  readonly isLoading: boolean
  readonly error?: string
}

interface NodeInfoRow {
  readonly label: string
  readonly value?: string
  readonly to?: string
  readonly isLoading?: boolean
}

export const HomePage: FC<HomePageProps> = ({client}) => {
  const navigate = useNavigate()
  const openPath = useOpenExplorerPath()
  const {showToast} = useToast()
  const {prefetchNames} = useAddressBook()
  const [nodeInfo, setNodeInfo] = useState<LocalnetNodeInfo | undefined>()
  const [copiedEndpoint, setCopiedEndpoint] = useState<string>()
  const [homeState, setHomeState] = useState<HomeState>({
    transactions: [],
    accountStatesByAddress: {},
    isLoading: true,
  })
  const endpoints = useMemo(() => client.getEndpoints(), [client])
  const endpointRows = useMemo(
    () =>
      [
        {
          label: "V2 API",
          value: endpoints.apiV2,
          referencePath: "/api-reference/v2",
        },
        {
          label: "V3 API",
          value: endpoints.apiV3,
          referencePath: "/api-reference/v3",
        },
        {
          label: "Control API",
          value: endpoints.admin,
          referencePath: "/api-reference/control",
        },
      ].filter(endpoint => endpoint.value.length > 0),
    [endpoints],
  )
  const nodeInfoRows = useMemo<readonly NodeInfoRow[]>(() => {
    const isLoading = nodeInfo === undefined

    return [
      {
        label: "Latest block",
        value: nodeInfo?.last_block_seqno.toString(),
        to: nodeInfo ? getMasterchainBlockPath(nodeInfo.last_block_seqno) : undefined,
        isLoading,
      },
      {
        label: "Uptime",
        value: nodeInfo ? formatDuration(nodeInfo.uptime_seconds) : undefined,
        isLoading,
      },
      {
        label: "State source",
        value: nodeInfo ? formatNodeInfoValue(nodeInfo.state_source) : undefined,
        isLoading,
      },
      {
        label: "Fork network",
        value: nodeInfo ? formatOptionalNodeInfoValue(nodeInfo.fork_network) : undefined,
        isLoading,
      },
      {
        label: "Fork block",
        value: nodeInfo
          ? formatOptionalNodeInfoValue(nodeInfo.fork_block_number?.toLocaleString())
          : undefined,
        isLoading,
      },
      {
        label: "Response delay",
        value: nodeInfo
          ? nodeInfo.network_conditions
            ? `${nodeInfo.network_conditions.response_delay_ms} ms`
            : "—"
          : undefined,
        isLoading,
      },
    ]
  }, [nodeInfo])
  const recentAccounts = useMemo(
    () => collectRecentAccounts(homeState.transactions),
    [homeState.transactions],
  )
  const recentAccountItems = useMemo<readonly DeveloperAccountListItem[]>(
    () =>
      recentAccounts.map(address => ({
        address,
        state: homeState.accountStatesByAddress[addressKey(address)],
      })),
    [homeState.accountStatesByAddress, recentAccounts],
  )
  const {addresses: displayedAddresses, messageNamesByAddress} = useTransactionMessageNames(
    client,
    homeState.transactions,
  )

  useEffect(() => {
    let cancelled = false
    let timeoutId: ReturnType<typeof setTimeout> | undefined

    const loadNodeInfo = async () => {
      try {
        const nextNodeInfo = await client.getNodeInfo()
        if (!cancelled) {
          setNodeInfo(nextNodeInfo)
        }
      } catch {
        if (!cancelled) {
          setNodeInfo(undefined)
        }
      } finally {
        if (!cancelled) {
          timeoutId = globalThis.setTimeout(() => void loadNodeInfo(), HOME_NODE_INFO_REFRESH_MS)
        }
      }
    }

    void loadNodeInfo()

    return () => {
      cancelled = true
      if (timeoutId !== undefined) {
        globalThis.clearTimeout(timeoutId)
      }
    }
  }, [client])

  useEffect(() => {
    let cancelled = false
    let timeoutId: ReturnType<typeof setTimeout> | undefined

    const loadHomeState = async (showLoading: boolean) => {
      if (showLoading) {
        setHomeState(current => ({
          ...current,
          isLoading: true,
          error: undefined,
        }))
      }

      try {
        const transactionsResponse = await client.getRecentTransactions(8)
        const transactions = transactionsResponse.transactions
        const accounts = collectRecentAccounts(transactions)
        let accountStatesByAddress: Record<string, V3AccountState> = {}

        if (accounts.length > 0) {
          try {
            const accountStates = await client.getAccountStates(accounts, false)
            accountStatesByAddress = Object.fromEntries(
              accountStates.accounts.map(account => [addressKey(account.address), account]),
            )
          } catch (error) {
            console.error("Failed to fetch recent account states", error)
          }
        }

        if (!cancelled) {
          setHomeState({
            transactions,
            accountStatesByAddress,
            isLoading: false,
          })
        }
      } catch (error) {
        if (!cancelled) {
          const message = error instanceof Error ? error.message : "Failed to load dashboard"
          setHomeState(current => ({
            transactions: current.transactions,
            accountStatesByAddress: current.accountStatesByAddress,
            isLoading: false,
            error: current.transactions.length === 0 ? message : undefined,
          }))
        }
      } finally {
        if (!cancelled) {
          timeoutId = globalThis.setTimeout(
            () => void loadHomeState(false),
            HOME_RECENT_TRANSACTIONS_REFRESH_MS,
          )
        }
      }
    }

    void loadHomeState(true)

    return () => {
      cancelled = true
      if (timeoutId !== undefined) {
        globalThis.clearTimeout(timeoutId)
      }
    }
  }, [client])

  useEffect(() => {
    void prefetchNames(displayedAddresses)
  }, [displayedAddresses, prefetchNames])

  useEffect(() => {
    if (!copiedEndpoint) {
      return
    }

    const timeoutId = globalThis.setTimeout(() => setCopiedEndpoint(undefined), 2000)
    return () => {
      globalThis.clearTimeout(timeoutId)
    }
  }, [copiedEndpoint])

  const copyEndpoint = useCallback(
    async (endpoint: string) => {
      try {
        await navigator.clipboard.writeText(endpoint)
        setCopiedEndpoint(endpoint)
      } catch (error) {
        console.error("Failed to copy endpoint", error)
        showToast({
          variant: "error",
          title: "Copy failed",
          description: "Failed to copy endpoint URL.",
        })
      }
    },
    [showToast],
  )

  return (
    <>
      <section className={styles.hero}>
        <div>
          <h1 className={styles.title}>Home</h1>
          <p className={styles.subtitle}>
            A quick snapshot of your local node and recent activity.
          </p>
        </div>
      </section>

      <section className={styles.homeLayout}>
        <div className={styles.homeTopRow}>
          <Card className={`${styles.dashboardCard} ${styles.homeCard}`}>
            <CardHeader className={styles.dashboardCardHeader}>
              <CardTitle className={styles.dashboardCardTitle}>Node info</CardTitle>
            </CardHeader>
            <CardContent className={`${styles.dashboardCardContent} ${styles.nodeInfoList}`}>
              {nodeInfoRows.map(row => {
                const value = row.value ?? "—"

                return (
                  <div key={row.label} className={styles.nodeInfoRow}>
                    <span className={styles.nodeInfoLabel}>{row.label}</span>
                    {row.isLoading ? (
                      <span
                        className={`${styles.skeletonLine} ${styles.nodeInfoValueSkeleton}`}
                        aria-label={`Loading ${row.label}`}
                      />
                    ) : row.to ? (
                      <Link className={styles.nodeInfoValueLink} to={row.to} title={value}>
                        {value}
                      </Link>
                    ) : (
                      <span className={styles.nodeInfoValue} title={value}>
                        {value}
                      </span>
                    )}
                  </div>
                )
              })}
            </CardContent>
          </Card>

          <Card className={`${styles.dashboardCard} ${styles.homeCard}`}>
            <CardHeader className={styles.dashboardCardHeader}>
              <CardTitle className={styles.dashboardCardTitle}>Endpoints</CardTitle>
            </CardHeader>
            <CardContent className={`${styles.dashboardCardContent} ${styles.endpointList}`}>
              {endpointRows.map(endpoint => {
                const isCopied = copiedEndpoint === endpoint.value

                return (
                  <div key={endpoint.label} className={styles.endpointRow}>
                    <span className={styles.endpointLabel}>{endpoint.label}</span>
                    <span className={styles.endpointValueRow}>
                      <span className={styles.endpointValue}>{endpoint.value}</span>
                      <span className={styles.endpointActions}>
                        <button
                          type="button"
                          className={`${styles.endpointButton} ${isCopied ? styles.endpointButtonCopied : ""}`}
                          aria-label={
                            isCopied ? "Endpoint copied" : `Copy ${endpoint.label} endpoint`
                          }
                          title={isCopied ? "Copied" : "Copy endpoint"}
                          onClick={() => void copyEndpoint(endpoint.value)}
                        >
                          {isCopied ? <Check size={14} /> : <Copy size={14} />}
                        </button>
                        <button
                          type="button"
                          className={styles.endpointButton}
                          aria-label={`Open ${endpoint.label} reference`}
                          title="Open API reference"
                          onClick={() => void navigate(endpoint.referencePath)}
                        >
                          <BookOpen size={14} />
                        </button>
                      </span>
                    </span>
                  </div>
                )
              })}
            </CardContent>
          </Card>
        </div>

        {homeState.error ? (
          <div className={`${styles.homeTransactionsCard} ${styles.emptyState}`}>
            {homeState.error}
          </div>
        ) : homeState.isLoading ? (
          <DeveloperTransactionListSkeleton
            className={styles.homeTransactionsCard}
            title="Recent transactions"
          />
        ) : homeState.transactions.length === 0 ? (
          <div className={`${styles.homeTransactionsCard} ${styles.emptyState}`}>
            No transactions yet
          </div>
        ) : (
          <DeveloperTransactionList
            className={styles.homeTransactionsCard}
            title="Recent transactions"
            transactions={homeState.transactions}
            messageNamesByAddress={messageNamesByAddress}
            onTransactionClick={(hashHex, _transaction, event) => {
              openPath(`/explorer/tx/${encodeURIComponent(hashHex)}`, event)
            }}
            onAddressClick={(address, event) => {
              openPath(`/explorer/address/${encodeURIComponent(address)}`, event)
            }}
          />
        )}

        <div className={styles.homeMainColumn}>
          {homeState.error ? (
            <div className={styles.emptyState}>{homeState.error}</div>
          ) : homeState.isLoading ? (
            <DeveloperAccountListSkeleton title="Recent accounts" />
          ) : (
            <DeveloperAccountList
              title="Recent accounts"
              accounts={recentAccountItems}
              onAddressClick={(address, event) => {
                openPath(`/explorer/address/${encodeURIComponent(address)}`, event)
              }}
            />
          )}
        </div>
      </section>
    </>
  )
}

function formatNodeInfoValue(value: string): string {
  const normalized = value.trim()
  if (normalized.length === 0) {
    return "—"
  }

  return normalized.replace(/_/g, " ")
}

function formatOptionalNodeInfoValue(value: string | null | undefined): string {
  if (value === undefined || value === null) {
    return "—"
  }

  return formatNodeInfoValue(value)
}

function getMasterchainBlockPath(seqno: number): string {
  return `/block/-1/${encodeURIComponent(MASTERCHAIN_BLOCK_SHARD)}/${seqno}`
}

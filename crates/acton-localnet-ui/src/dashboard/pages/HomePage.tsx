import {
  ArrowUpRight,
  ChartNoAxesColumn,
  CircleUserRound,
  Link2,
  SquareStack,
  Wallet,
} from "lucide-react"
import * as React from "react"
import {Card, CardContent, CardDescription, CardHeader, CardTitle} from "@acton/shared-ui"
import {useNavigate} from "react-router-dom"

import type {TonClient} from "../../explorer/api/client"
import type {LocalnetNodeInfo, V3TransactionListItem} from "../../explorer/api/types"
import {formatDuration, formatNano, formatTimeAgo, hashToHex} from "../../explorer/components/utils"
import {collectRecentAccounts} from "../dashboardUtils"
import {HomeAddressLabel} from "../HomeAddressLabel"

import styles from "../DashboardPage.module.css"

interface HomePageProps {
  readonly client: TonClient
}

interface HomeState {
  readonly nodeInfo?: LocalnetNodeInfo
  readonly transactions: readonly V3TransactionListItem[]
  readonly accountBalances: Readonly<Record<string, string>>
  readonly isLoading: boolean
  readonly error?: string
}

export const HomePage: React.FC<HomePageProps> = ({client}) => {
  const navigate = useNavigate()
  const [homeState, setHomeState] = React.useState<HomeState>({
    transactions: [],
    accountBalances: {},
    isLoading: true,
  })
  const endpoints = React.useMemo(() => client.getEndpoints(), [client])
  const endpointRows = React.useMemo(
    () =>
      [
        {label: "V2 API", value: endpoints.apiV2},
        {label: "V3 API", value: endpoints.apiV3},
      ].filter(endpoint => endpoint.value.length > 0),
    [endpoints],
  )
  const recentAccounts = React.useMemo(
    () => collectRecentAccounts(homeState.transactions),
    [homeState.transactions],
  )

  React.useEffect(() => {
    let cancelled = false

    void (async () => {
      setHomeState(current => ({
        ...current,
        isLoading: true,
        error: undefined,
      }))

      try {
        const [nodeInfo, transactionsResponse] = await Promise.all([
          client.getNodeInfo(),
          client.getRecentTransactions(8),
        ])
        const transactions = transactionsResponse.transactions
        const accounts = collectRecentAccounts(transactions)
        let accountBalances: Record<string, string> = {}

        if (accounts.length > 0) {
          try {
            const accountStates = await client.getAccountStates(accounts, false)
            accountBalances = Object.fromEntries(
              accountStates.accounts.map(account => [account.address, account.balance]),
            )
          } catch (error) {
            console.error("Failed to fetch recent account balances", error)
          }
        }

        if (cancelled) {
          return
        }

        setHomeState({
          nodeInfo,
          transactions,
          accountBalances,
          isLoading: false,
        })
      } catch (error) {
        if (cancelled) {
          return
        }

        setHomeState({
          transactions: [],
          accountBalances: {},
          isLoading: false,
          error: error instanceof Error ? error.message : "Failed to load dashboard",
        })
      }
    })()

    return () => {
      cancelled = true
    }
  }, [client])

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
        <div className={styles.homeMainColumn}>
          <Card className={`${styles.dashboardCard} ${styles.homeCard}`}>
            <CardHeader className={styles.dashboardCardHeader}>
              <div className={styles.cardTitleRow}>
                <div className={styles.cardIcon}>
                  <ChartNoAxesColumn size={16} />
                </div>
                <div>
                  <CardTitle className={styles.dashboardCardTitle}>Recent transactions</CardTitle>
                  <CardDescription className={styles.dashboardCardDescription}>
                    Fresh activity from the local network.
                  </CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent className={styles.dashboardCardContent}>
              {homeState.error ? (
                <div className={styles.emptyState}>{homeState.error}</div>
              ) : homeState.isLoading ? (
                <div className={styles.emptyState}>Loading transactions…</div>
              ) : homeState.transactions.length === 0 ? (
                <div className={styles.emptyState}>No transactions yet.</div>
              ) : (
                <div className={styles.listTable}>
                  {homeState.transactions.map(transaction => (
                    <button
                      key={transaction.hash}
                      type="button"
                      className={styles.listRowButton}
                      onClick={() => {
                        const hashHex = hashToHex(transaction.hash)
                        if (hashHex) {
                          void navigate(`/explorer/tx/${encodeURIComponent(hashHex)}`)
                        }
                      }}
                    >
                      <div className={styles.rowMain}>
                        <div className={styles.rowTopLine}>
                          <div className={styles.rowPrimary}>
                            <HomeAddressLabel address={transaction.account} />
                          </div>
                          {transaction.description.compute_ph.success ? undefined : (
                            <span className={`${styles.statusBadge} ${styles.statusError}`}>
                              Exit {transaction.description.compute_ph.exit_code}
                            </span>
                          )}
                        </div>
                        <div className={styles.rowSecondaryDetail}>
                          From:{" "}
                          <HomeAddressLabel
                            address={transaction.in_msg?.source}
                            className={styles.rowInlineAddress}
                          />{" "}
                          · Value: {formatNano(transaction.in_msg?.value || "0")} TON
                        </div>
                      </div>
                      <div className={styles.rowMeta}>
                        <div className={styles.rowPrimary}>#{transaction.mc_block_seqno}</div>
                        <div className={styles.rowSecondary}>{formatTimeAgo(transaction.now)}</div>
                      </div>
                    </button>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>

          <Card className={`${styles.dashboardCard} ${styles.homeCard}`}>
            <CardHeader className={styles.dashboardCardHeader}>
              <div className={styles.cardTitleRow}>
                <div className={styles.cardIcon}>
                  <Wallet size={16} />
                </div>
                <div>
                  <CardTitle className={styles.dashboardCardTitle}>Recent accounts</CardTitle>
                  <CardDescription className={styles.dashboardCardDescription}>
                    Accounts touched by the latest transactions.
                  </CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent className={styles.dashboardCardContent}>
              {homeState.error ? (
                <div className={styles.emptyState}>{homeState.error}</div>
              ) : homeState.isLoading ? (
                <div className={styles.emptyState}>Loading accounts…</div>
              ) : recentAccounts.length === 0 ? (
                <div className={styles.emptyState}>No accounts yet.</div>
              ) : (
                <div className={styles.accountList}>
                  {recentAccounts.map(account => {
                    const balance = homeState.accountBalances[account]

                    return (
                      <button
                        key={account}
                        type="button"
                        className={styles.accountItem}
                        onClick={() => {
                          void navigate(`/explorer/address/${encodeURIComponent(account)}`)
                        }}
                      >
                        <span className={styles.accountIcon}>
                          <CircleUserRound size={14} />
                        </span>
                        <span className={styles.accountText}>
                          <span className={styles.accountName}>
                            <HomeAddressLabel address={account} />
                          </span>
                          <span className={styles.accountBalance}>
                            {balance === undefined
                              ? "Balance unavailable"
                              : `${formatNano(balance)} TON`}
                          </span>
                        </span>
                        <ArrowUpRight size={14} className={styles.accountArrow} />
                      </button>
                    )
                  })}
                </div>
              )}
            </CardContent>
          </Card>
        </div>

        <aside className={styles.homeSideColumn}>
          <Card className={`${styles.dashboardCard} ${styles.homeCard}`}>
            <CardHeader className={styles.dashboardCardHeader}>
              <div className={styles.cardTitleRow}>
                <div className={styles.cardIcon}>
                  <SquareStack size={16} />
                </div>
                <div>
                  <CardTitle className={styles.dashboardCardTitle}>Current block</CardTitle>
                  <CardDescription className={styles.dashboardCardDescription}>
                    Latest masterchain sequence number.
                  </CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent className={styles.dashboardCardContent}>
              <div className={styles.metricValue}>
                {homeState.nodeInfo ? `#${homeState.nodeInfo.last_block_seqno}` : "—"}
              </div>
              <div className={styles.metricMeta}>
                {homeState.nodeInfo
                  ? `${formatDuration(homeState.nodeInfo.uptime_seconds)} uptime`
                  : "Waiting for node info"}
              </div>
            </CardContent>
          </Card>

          <Card className={`${styles.dashboardCard} ${styles.homeCard}`}>
            <CardHeader className={styles.dashboardCardHeader}>
              <div className={styles.cardTitleRow}>
                <div className={styles.cardIcon}>
                  <Link2 size={16} />
                </div>
                <div>
                  <CardTitle className={styles.dashboardCardTitle}>Endpoints</CardTitle>
                  <CardDescription className={styles.dashboardCardDescription}>
                    Active local URLs for the current node.
                  </CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent className={`${styles.dashboardCardContent} ${styles.endpointList}`}>
              {endpointRows.map(endpoint => (
                <a
                  key={endpoint.label}
                  className={styles.endpointRow}
                  href={endpoint.value}
                  target="_blank"
                  rel="noreferrer"
                >
                  <span className={styles.endpointText}>
                    <span className={styles.endpointLabel}>{endpoint.label}</span>
                    <span className={styles.endpointValue}>{endpoint.value}</span>
                  </span>
                  <span className={styles.endpointAction}>
                    <ArrowUpRight size={14} />
                  </span>
                </a>
              ))}
            </CardContent>
          </Card>
        </aside>
      </section>
    </>
  )
}

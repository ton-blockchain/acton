import {
  ArrowUpRight,
  Bot,
  Boxes,
  ChartNoAxesColumn,
  ChevronRight,
  ChevronsUpDown,
  Database,
  Flag,
  Globe,
  Link2,
  LayoutGrid,
  Logs,
  Plug,
  Rocket,
  Search,
  Shield,
  SquareStack,
  Variable,
  Wallet,
} from "lucide-react"
import * as React from "react"
import {Button, Card, CardContent, CardDescription, CardHeader, CardTitle, Input, useToast} from "@acton/shared-ui"
import type {LucideIcon} from "lucide-react"
import {useLocation, useNavigate} from "react-router-dom"

import type {TonClient} from "../explorer/api/client"
import type {LocalnetNodeInfo, V3TransactionListItem} from "../explorer/api/types"

import {formatAddress, formatNano, formatTimeAgo, hashToHex, parseAddress} from "../explorer/components/utils"

import styles from "./DashboardPage.module.css"

interface SidebarItem {
  readonly label: string
  readonly icon: LucideIcon
  readonly expandable?: boolean
  readonly disabled?: boolean
  readonly path?: string
}

const mainItems: SidebarItem[] = [
  {label: "Home", icon: LayoutGrid, path: "/dashboard"},
  {label: "Faucet", icon: Wallet, path: "/dashboard/faucet"},
  {label: "Projects", icon: LayoutGrid},
  {label: "Deployments", icon: Rocket},
  {label: "Logs", icon: Logs},
  {label: "Analytics", icon: ChartNoAxesColumn},
  {label: "Speed Insights", icon: ChartNoAxesColumn},
  {label: "Firewall", icon: Shield},
  {label: "CDN", icon: Globe},
]

const resourceItems: SidebarItem[] = [
  {label: "Environment Variables", icon: Variable},
  {label: "Domains", icon: Boxes},
  {label: "Integrations", icon: Plug},
  {label: "Storage", icon: Database},
  {label: "Flags", icon: Flag},
  {label: "Agent", icon: Bot, expandable: true},
  {label: "AI Gateway", icon: ChevronsUpDown, expandable: true, disabled: true},
]

const quickAmounts = ["1", "5", "20", "100"]

interface DashboardPageProps {
  readonly client: TonClient
}

interface HomeState {
  readonly nodeInfo?: LocalnetNodeInfo
  readonly transactions: readonly V3TransactionListItem[]
  readonly isLoading: boolean
  readonly error?: string
}

function parseTonAmount(value: string): number | undefined {
  const trimmed = value.trim()
  if (!trimmed || !/^\d+(\.\d{0,9})?$/.test(trimmed)) {
    return undefined
  }

  const [wholePart, fractionPart = ""] = trimmed.split(".")
  const whole = BigInt(wholePart)
  const fraction = BigInt(fractionPart.padEnd(9, "0"))
  const nano = whole * 1_000_000_000n + fraction
  if (nano <= 0n || nano > BigInt(Number.MAX_SAFE_INTEGER)) {
    return undefined
  }
  return Number(nano)
}

export const DashboardPage: React.FC<DashboardPageProps> = ({client}) => {
  const location = useLocation()
  const navigate = useNavigate()
  const {showToast} = useToast()
  const [address, setAddress] = React.useState("")
  const [amount, setAmount] = React.useState("1")
  const [isSubmitting, setIsSubmitting] = React.useState(false)
  const [homeState, setHomeState] = React.useState<HomeState>({
    transactions: [],
    isLoading: true,
  })

  const amountNano = React.useMemo(() => parseTonAmount(amount), [amount])
  const isFaucetPage = location.pathname === "/dashboard/faucet"
  const isHomePage = location.pathname === "/dashboard"
  const endpoints = React.useMemo(() => client.getEndpoints(), [client])
  const endpointRows = React.useMemo(
    () =>
      [
        {label: "V2 API", value: endpoints.apiV2},
        {label: "V3 API", value: endpoints.apiV3},
      ].filter(endpoint => endpoint.value.length > 0),
    [endpoints],
  )
  const recentAccounts = React.useMemo(() => {
    const seen = new Set<string>()
    const accounts: string[] = []

    for (const transaction of homeState.transactions) {
      if (!seen.has(transaction.account)) {
        seen.add(transaction.account)
        accounts.push(transaction.account)
      }
      if (accounts.length === 6) {
        break
      }
    }

    return accounts
  }, [homeState.transactions])

  React.useEffect(() => {
    if (!isHomePage) {
      return
    }

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

        if (cancelled) {
          return
        }

        setHomeState({
          nodeInfo,
          transactions: transactionsResponse.transactions,
          isLoading: false,
        })
      } catch (error) {
        if (cancelled) {
          return
        }

        setHomeState({
          transactions: [],
          isLoading: false,
          error: error instanceof Error ? error.message : "Failed to load dashboard",
        })
      }
    })()

    return () => {
      cancelled = true
    }
  }, [client, isHomePage])

  async function handleSubmit(event?: React.FormEvent): Promise<void> {
    event?.preventDefault()
    const trimmedAddress = address.trim()
    const parsedAddress = parseAddress(trimmedAddress)
    if (!parsedAddress) {
      showToast({
        variant: "error",
        title: "Invalid address",
        description: "Enter a valid TON address.",
      })
      return
    }
    if (amountNano === undefined) {
      showToast({
        variant: "error",
        title: "Invalid amount",
        description: "Enter a valid amount greater than zero.",
      })
      return
    }

    const normalized = parsedAddress.toString({testOnly: true})
    setIsSubmitting(true)

    try {
      await client.fundAccount(normalized, amountNano)
      showToast({
        variant: "success",
        title: "Transfer sent",
        description: `Sent ${amount.trim()} TON to ${formatAddress(normalized)}.`,
      })
    } catch (submitError) {
      showToast({
        variant: "error",
        title: "Transfer failed",
        description: submitError instanceof Error ? submitError.message : "Failed to send TON.",
      })
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <div className={styles.page}>
      <aside className={styles.sidebar}>
        <div className={styles.topControls}>
          <button type="button" className={styles.searchButton}>
            <div className={styles.searchButtonValue}>
              <Search size={16} />
              <span>Find...</span>
            </div>
            <span className={styles.searchShortcut}>F</span>
          </button>
        </div>

        <div className={styles.navScroll}>
          <nav className={styles.nav}>
            <div className={styles.navSection}>
              {mainItems.map(item => {
                const Icon = item.icon

                return (
                  <button
                    type="button"
                    key={item.label}
                    className={`${styles.navItem} ${
                      item.path === location.pathname ? styles.navItemActive : ""
                    }`}
                    disabled={!item.path}
                    onClick={() => {
                      if (item.path) {
                        void navigate(item.path)
                      }
                    }}
                  >
                    <span className={styles.navItemMain}>
                      <Icon size={18} />
                      <span>{item.label}</span>
                    </span>
                    {item.expandable ? <ChevronRight size={14} /> : undefined}
                  </button>
                )
              })}
            </div>

            <div className={styles.navDivider} />

            <div className={styles.navSection}>
              {resourceItems.map(item => {
                const Icon = item.icon

                return (
                  <button
                    type="button"
                    key={item.label}
                    className={`${styles.navItem} ${item.disabled ? styles.navItemDisabled : ""}`}
                  >
                    <span className={styles.navItemMain}>
                      <Icon size={18} />
                      <span>{item.label}</span>
                    </span>
                    {item.expandable ? <ChevronRight size={14} /> : undefined}
                  </button>
                )
              })}
            </div>
          </nav>
        </div>
      </aside>

      <section className={styles.contentArea}>
        <main className={styles.content}>
          {isHomePage ? (
            <>
              <section className={styles.hero}>
                <div>
                  <h1 className={styles.title}>Home</h1>
                  <p className={styles.subtitle}>A quick snapshot of your local node and recent activity.</p>
                </div>
              </section>

              <section className={styles.homeGrid}>
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
                      {homeState.nodeInfo ? `${homeState.nodeInfo.uptime_seconds}s uptime` : "Waiting for node info"}
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
                      <div key={endpoint.label} className={styles.endpointRow}>
                        <span className={styles.endpointLabel}>{endpoint.label}</span>
                        <code className={styles.endpointValue}>{endpoint.value}</code>
                      </div>
                    ))}
                  </CardContent>
                </Card>

                <Card className={`${styles.dashboardCard} ${styles.homeCard} ${styles.homeCardWide}`}>
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
                                <div className={styles.rowPrimary}>{formatAddress(transaction.account)}</div>
                                {transaction.description.compute_ph.success ? undefined : (
                                  <span className={`${styles.statusBadge} ${styles.statusError}`}>
                                    Exit {transaction.description.compute_ph.exit_code}
                                  </span>
                                )}
                              </div>
                              <div className={styles.rowSecondaryDetail}>
                                From: {formatAddress(transaction.in_msg?.source || "Unknown")} · Value:{" "}
                                {formatNano(transaction.in_msg?.value || "0")} TON
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
                        {recentAccounts.map(account => (
                          <button
                            key={account}
                            type="button"
                            className={styles.accountItem}
                            onClick={() => {
                              void navigate(`/explorer/address/${encodeURIComponent(account)}`)
                            }}
                          >
                            <span className={styles.accountDot} />
                            <span>{formatAddress(account)}</span>
                          </button>
                        ))}
                      </div>
                    )}
                  </CardContent>
                </Card>
              </section>
            </>
          ) : undefined}

          {isFaucetPage ? (
            <>
              <section className={styles.hero}>
                <div>
                  <h1 className={styles.title}>Send test TON</h1>
                  <p className={styles.subtitle}>Top up any wallet address with test TON in a few seconds.</p>
                </div>
              </section>

              <section className={styles.faucetLayout}>
                <form className={styles.formCard} onSubmit={event => void handleSubmit(event)}>
                  <div className={styles.cardHeader}>
                    <div className={styles.cardTitleRow}>
                      <div className={styles.cardIcon}>
                        <Wallet size={16} />
                      </div>
                      <div>
                        <h2 className={styles.cardTitle}>Wallet top up</h2>
                        <p className={styles.cardDescription}>Enter an address, choose an amount, and send funds.</p>
                      </div>
                    </div>
                  </div>

                  <div className={styles.fieldBlock}>
                    <label className={styles.label} htmlFor="dashboard-address">
                      Recipient address
                    </label>
                    <Input
                      id="dashboard-address"
                      className={styles.fieldInput}
                      placeholder="EQ..."
                      value={address}
                      onChange={event => setAddress(event.target.value)}
                    />
                    <p className={styles.hint}>Paste any raw or user-friendly TON address.</p>
                  </div>

                  <div className={styles.fieldBlock}>
                    <label className={styles.label} htmlFor="dashboard-amount">
                      Amount
                    </label>
                    <div className={styles.amountRow}>
                  <Input
                    id="dashboard-amount"
                    className={styles.fieldInput}
                    inputMode="decimal"
                    placeholder="0.0 TON"
                    value={amount}
                    onChange={event => setAmount(event.target.value)}
                  />
                    </div>
                  </div>
                    <div className={styles.quickActions}>
                      {quickAmounts.map(value => (
                        <Button
                          key={value}
                          type="button"
                          variant={amount === value ? "secondary" : "outline"}
                          size="sm"
                          className={styles.quickActionButton}
                          onClick={() => setAmount(value)}
                        >
                          {value} TON
                        </Button>
                      ))}
                    </div>

                  <div className={styles.formFooter}>
                    <div className={styles.formHint}>Use this faucet to fund wallets for testing.</div>
                    <Button
                      type="submit"
                      className={styles.sendButton}
                      disabled={isSubmitting || address.trim().length === 0 || amount.trim().length === 0}
                    >
                      <span>{isSubmitting ? "Sending..." : "Send TON"}</span>
                      <ArrowUpRight size={16} />
                    </Button>
                  </div>
                </form>
              </section>
            </>
          ) : undefined}
        </main>
      </section>
    </div>
  )
}

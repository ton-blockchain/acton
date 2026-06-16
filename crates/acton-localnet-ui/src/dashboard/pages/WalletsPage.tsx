import {
  ArrowUpRight,
  Check,
  Copy,
  Link2,
  RefreshCw,
  Unplug,
  Wallet as WalletIcon,
} from "lucide-react"
import * as React from "react"
import {Button, Card, CardContent, CardDescription, CardHeader, CardTitle} from "@acton/shared-ui"
import {formatUnits} from "@ton/walletkit"
import {Link} from "react-router-dom"

import {formatAddress, normalizeAddress} from "../../explorer/components/utils"
import {useAddressFormat} from "../../explorer/hooks/useNetworkInfo"
import type {RuntimeWallet} from "../../wallet/types"
import {useWalletRuntime, type WalletBalanceState} from "../../wallet/useWalletRuntime"
import dashboardStyles from "../DashboardPage.module.css"

import styles from "./WalletsPage.module.css"

export const WalletsPage: React.FC = () => {
  const addressFormat = useAddressFormat()
  const {
    host,
    runtimeWallets,
    unsupportedWallets,
    sessions,
    walletBalances,
    copiedAddress,
    tonConnectUrl,
    isLoadingWallets,
    isInitializing,
    isSyncingWallets,
    isSubmitting,
    isRefreshingBalances,
    pendingRequestCount,
    setTonConnectUrl,
    handleConnectUrl,
    refreshWalletBalances,
    handleDisconnectSession,
    handleCopyAddress,
  } = useWalletRuntime()

  const handleConnectUrlSubmit = async (event: React.FormEvent) => {
    event.preventDefault()
    if (tonConnectUrl.trim().length === 0) {
      return
    }

    await handleConnectUrl(tonConnectUrl)
  }
  const isBusy = isLoadingWallets || isInitializing || isSyncingWallets

  return (
    <>
      <section className={dashboardStyles.hero}>
        <div>
          <h1 className={dashboardStyles.title}>Wallets</h1>
          <p className={dashboardStyles.subtitle}>
            Startup wallets from this localnet, ready for TON Connect approvals.
          </p>
        </div>
      </section>

      <section className={styles.walletLayout}>
        <div className={styles.walletTopGrid}>
          <Card className={`${dashboardStyles.dashboardCard} ${styles.walletListCard}`}>
            <CardHeader
              className={`${dashboardStyles.dashboardCardHeader} ${styles.walletCardHeader}`}
            >
              <div className={styles.walletHeader}>
                <div className={`${dashboardStyles.cardTitleRow} ${styles.walletHeaderTitle}`}>
                  <div className={dashboardStyles.cardIcon}>
                    <WalletIcon size={16} />
                  </div>
                  <div>
                    <CardTitle className={dashboardStyles.dashboardCardTitle}>
                      Startup wallets
                    </CardTitle>
                    <CardDescription className={dashboardStyles.dashboardCardDescription}>
                      Wallets selected with localnet startup accounts.
                    </CardDescription>
                  </div>
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => void refreshWalletBalances()}
                  disabled={runtimeWallets.length === 0 || isRefreshingBalances}
                >
                  <RefreshCw size={14} className={isRefreshingBalances ? styles.spinning : ""} />
                  Refresh
                </Button>
              </div>
            </CardHeader>
            <CardContent
              className={`${dashboardStyles.dashboardCardContent} ${styles.walletCardContent}`}
            >
              {isBusy ? (
                <div className={dashboardStyles.emptyState}>Loading wallets...</div>
              ) : runtimeWallets.length === 0 ? (
                <div className={dashboardStyles.emptyState}>
                  No supported startup wallets. Start localnet with `--accounts` or
                  `[localnet].accounts`.
                </div>
              ) : (
                <div className={styles.walletList}>
                  {runtimeWallets.map(wallet => {
                    const balanceState = walletBalances[wallet.id]
                    const walletAddress = normalizeAddress(wallet.record.address, addressFormat)

                    return (
                      <article key={wallet.id} className={styles.walletRow}>
                        <div className={styles.walletSummary}>
                          <span className={styles.walletBody}>
                            <span className={styles.walletTopLine}>
                              <span className={styles.walletTitleGroup}>
                                <span className={styles.walletName}>{wallet.record.name}</span>
                                <span className={styles.walletBadge}>
                                  {wallet.record.version.toUpperCase()}
                                </span>
                              </span>
                            </span>
                            <span className={styles.walletDetailsLine}>
                              <span className={styles.walletAddressCluster}>
                                <span className={styles.walletAddress}>
                                  {formatAddress(walletAddress, true, addressFormat)}
                                </span>
                                <button
                                  type="button"
                                  className={`${styles.walletInlineAction} ${
                                    copiedAddress === walletAddress
                                      ? styles.walletInlineActionActive
                                      : ""
                                  }`}
                                  onClick={() => void handleCopyAddress(walletAddress)}
                                  aria-label={
                                    copiedAddress === walletAddress
                                      ? "Address copied"
                                      : "Copy address"
                                  }
                                >
                                  {copiedAddress === walletAddress ? (
                                    <Check size={13} />
                                  ) : (
                                    <Copy size={13} />
                                  )}
                                </button>
                                <Link
                                  to={`/explorer/address/${walletAddress}`}
                                  className={styles.walletInlineAction}
                                  aria-label={`Open ${wallet.record.name} in Explorer`}
                                >
                                  <ArrowUpRight size={13} />
                                </Link>
                              </span>
                            </span>
                          </span>
                        </div>
                        <span className={styles.walletBalance}>
                          {formatWalletBalanceLabel(balanceState)}
                        </span>
                      </article>
                    )
                  })}
                </div>
              )}

              {unsupportedWallets.length > 0 && (
                <div className={styles.unsupportedBlock}>
                  <div className={styles.unsupportedTitle}>Unsupported in WalletKit</div>
                  <div className={styles.unsupportedList}>
                    {unsupportedWallets.map(wallet => (
                      <span key={wallet.name} className={styles.unsupportedItem}>
                        {wallet.name} · {wallet.version}
                      </span>
                    ))}
                  </div>
                </div>
              )}
            </CardContent>
          </Card>

          <aside className={styles.sideColumn}>
            <Card className={`${dashboardStyles.dashboardCard} ${styles.sessionsCard}`}>
              <CardHeader className={dashboardStyles.dashboardCardHeader}>
                <CardTitle className={dashboardStyles.dashboardCardTitle}>Sessions</CardTitle>
                <CardDescription className={dashboardStyles.dashboardCardDescription}>
                  {pendingRequestCount === 0
                    ? "No pending approvals."
                    : `${pendingRequestCount} pending approval${pendingRequestCount === 1 ? "" : "s"}.`}
                </CardDescription>
              </CardHeader>
              <CardContent
                className={`${dashboardStyles.dashboardCardContent} ${styles.sessionsContent}`}
              >
                {sessions.length === 0 ? (
                  <div className={dashboardStyles.emptyState}>No active TON Connect sessions.</div>
                ) : (
                  <div className={styles.sessionList}>
                    {sessions.map(session => (
                      <article key={session.sessionId} className={styles.sessionCard}>
                        <div className={styles.sessionHeader}>
                          <div>
                            <div className={styles.sessionTitle}>
                              {getDappName(session.dAppName)}
                            </div>
                            <div className={styles.sessionDomain}>{session.domain}</div>
                          </div>
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            onClick={() => void handleDisconnectSession(session.sessionId)}
                            disabled={isSubmitting}
                          >
                            <Unplug size={14} />
                            Disconnect
                          </Button>
                        </div>
                        <MetaRow label="Wallet">
                          {findWalletName(runtimeWallets, session.walletId)}
                        </MetaRow>
                        <MetaRow label="Last activity">
                          {formatDateTime(session.lastActivityAt)}
                        </MetaRow>
                      </article>
                    ))}
                  </div>
                )}
              </CardContent>
            </Card>
          </aside>
        </div>

        <Card className={`${dashboardStyles.dashboardCard} ${styles.connectCard}`}>
          <CardHeader className={dashboardStyles.dashboardCardHeader}>
            <div className={dashboardStyles.cardTitleRow}>
              <div className={dashboardStyles.cardIcon}>
                <Link2 size={16} />
              </div>
              <div>
                <CardTitle className={dashboardStyles.dashboardCardTitle}>TON Connect</CardTitle>
                <CardDescription className={dashboardStyles.dashboardCardDescription}>
                  Paste a connect link from a local dApp, then approve it with a startup wallet.
                </CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className={dashboardStyles.dashboardCardContent}>
            <form
              className={styles.connectForm}
              onSubmit={event => void handleConnectUrlSubmit(event)}
            >
              <label className={styles.label} htmlFor="ton-connect-url">
                Connect URL
              </label>
              <textarea
                id="ton-connect-url"
                className={styles.textarea}
                rows={4}
                value={tonConnectUrl}
                onChange={event => setTonConnectUrl(event.target.value)}
                placeholder="tonconnect://..."
                disabled={runtimeWallets.length === 0 || isSubmitting}
              />
              <div className={styles.formFooter}>
                <span className={styles.helperText}>
                  Listening on {formatHostLabel(host)}. Paste also works anywhere in Localnet UI.
                </span>
                <Button
                  type="submit"
                  disabled={
                    runtimeWallets.length === 0 || tonConnectUrl.trim().length === 0 || isSubmitting
                  }
                >
                  <Link2 size={16} />
                  Handle request
                </Button>
              </div>
            </form>
          </CardContent>
        </Card>
      </section>
    </>
  )
}

interface MetaRowProps {
  readonly label: string
  readonly children: React.ReactNode
}

const MetaRow: React.FC<MetaRowProps> = ({label, children}) => (
  <div className={styles.metaRow}>
    <span className={styles.metaLabel}>{label}</span>
    <span className={styles.metaValue}>{children}</span>
  </div>
)

function formatGramBalance(balance: string): string {
  return formatUnits(balance, 9)
}

function formatCompactGramBalance(balance: string): string {
  const numericBalance = Number(formatGramBalance(balance))

  if (!Number.isFinite(numericBalance)) {
    return formatGramBalance(balance)
  }

  if (numericBalance > 0 && numericBalance < 0.0001) {
    return "<0.0001"
  }

  return numericBalance.toLocaleString(undefined, {
    maximumFractionDigits: 4,
  })
}

function formatWalletBalanceLabel(balanceState: WalletBalanceState | undefined): string {
  if (!balanceState) {
    return "Loading balance..."
  }

  if (balanceState.value) {
    const balance = `${formatCompactGramBalance(balanceState.value)} GRAM`
    return balanceState.isLoading ? `${balance} · updating` : balance
  }

  if (balanceState.isLoading) {
    return "Loading balance..."
  }

  return balanceState.error ? "Balance unavailable" : "Balance not loaded"
}

function formatDateTime(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) {
    return value
  }

  return date.toLocaleString()
}

function getDappName(name: string | undefined): string {
  return name && name.trim().length > 0 ? name : "Unknown dApp"
}

function findWalletName(wallets: readonly RuntimeWallet[], walletId: string): string {
  return wallets.find(wallet => wallet.id === walletId)?.record.name ?? "Unknown wallet"
}

function formatHostLabel(host: string): string {
  if (host.length === 0 && globalThis.location !== undefined) {
    return globalThis.location.host
  }

  try {
    return new URL(host).host
  } catch {
    return host || "current host"
  }
}

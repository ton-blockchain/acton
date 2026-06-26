import {useCallback, useEffect, useRef, useState} from "react"
import type {FC, FormEvent, JSX} from "react"
import {Link2, RefreshCw, Unplug} from "lucide-react"
import {Button} from "@acton/shared-ui"
import {formatUnits} from "@ton/walletkit"

import type {TonClient} from "../../explorer/api/client"
import type {JettonMasterMetadata, JettonWallet} from "../../explorer/api/types"
import {AddressChip} from "../../explorer/components/AddressChip"
import {
  TOKEN_IMAGE_SOURCE_KEYS,
  getImageSources,
  getPrimaryImageSource,
  replaceBrokenImageWithFallback,
} from "../../explorer/components/imageFallbacks"
import {
  normalizeAddress,
  toRawAddress,
  type AddressFormatOptions,
} from "../../explorer/components/utils"
import {useAddressFormat} from "../../explorer/hooks/useNetworkInfo"
import {
  useOpenExplorerPath,
  type ExplorerNavigationClickEvent,
} from "../../explorer/hooks/useOpenExplorerPath"
import type {RuntimeWallet} from "../../wallet/types"
import {useWalletRuntime, type WalletBalanceState} from "../../wallet/useWalletRuntime"
import dashboardStyles from "../DashboardPage.module.css"

import styles from "./WalletsPage.module.css"

interface WalletsPageProps {
  readonly client: TonClient
}

type WalletTokensById = Readonly<Record<string, readonly JettonWallet[]>>

const TOKEN_PREVIEW_LIMIT = 5

export const WalletsPage: FC<WalletsPageProps> = ({client}) => {
  const addressFormat = useAddressFormat()
  const openPath = useOpenExplorerPath()
  const [walletTokensById, setWalletTokensById] = useState<WalletTokensById>({})
  const [walletTokensLoading, setWalletTokensLoading] = useState(false)
  const walletTokensRequestRef = useRef(0)
  const {
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

  const handleConnectUrlSubmit = async (event: FormEvent) => {
    event.preventDefault()
    if (tonConnectUrl.trim().length === 0) {
      return
    }

    await handleConnectUrl(tonConnectUrl)
  }
  const isBusy = isLoadingWallets || isInitializing || isSyncingWallets
  const loadWalletTokens = useCallback(
    async (wallets: readonly RuntimeWallet[]) => {
      const requestId = walletTokensRequestRef.current + 1
      walletTokensRequestRef.current = requestId
      if (wallets.length === 0) {
        setWalletTokensById({})
        setWalletTokensLoading(false)
        return
      }

      setWalletTokensLoading(true)
      try {
        const ownerByRawAddress = new Map<string, string>()
        const ownerAddresses = wallets.map(wallet => {
          const walletAddress = normalizeAddress(wallet.record.address, addressFormat)
          ownerByRawAddress.set(toRawAddress(walletAddress), wallet.id)
          return walletAddress
        })
        const tokenWallets = await client.getJettonWallets(ownerAddresses)
        const missingJettonAddresses = new Set<string>()
        for (const tokenWallet of tokenWallets) {
          if (!tokenWallet.master) {
            missingJettonAddresses.add(tokenWallet.jetton)
          }
        }
        const missingMasters =
          missingJettonAddresses.size > 0
            ? await client.getJettonMasters([...missingJettonAddresses])
            : []
        const missingMastersByAddress = new Map(
          missingMasters.map(master => [toRawAddress(master.address), master] as const),
        )
        const nextTokensById: Record<string, JettonWallet[]> = {}
        for (const wallet of wallets) {
          nextTokensById[wallet.id] = []
        }
        for (const tokenWallet of tokenWallets) {
          const walletId = ownerByRawAddress.get(toRawAddress(tokenWallet.owner))
          if (!walletId) {
            continue
          }
          nextTokensById[walletId].push({
            ...tokenWallet,
            master:
              tokenWallet.master ?? missingMastersByAddress.get(toRawAddress(tokenWallet.jetton)),
          })
        }
        for (const [walletId, tokenWalletsForWallet] of Object.entries(nextTokensById)) {
          nextTokensById[walletId] = sortJettonWalletsByAmount(tokenWalletsForWallet)
        }
        if (walletTokensRequestRef.current === requestId) {
          setWalletTokensById(nextTokensById)
        }
      } catch (error) {
        if (walletTokensRequestRef.current === requestId) {
          console.error("Failed to fetch wallet token balances", error)
          setWalletTokensById({})
        }
      } finally {
        if (walletTokensRequestRef.current === requestId) {
          setWalletTokensLoading(false)
        }
      }
    },
    [addressFormat, client],
  )

  useEffect(() => {
    void loadWalletTokens(runtimeWallets)
  }, [loadWalletTokens, runtimeWallets])

  const handleRefreshWallets = async () => {
    await Promise.all([refreshWalletBalances(), loadWalletTokens(runtimeWallets)])
  }

  return (
    <>
      <section className={dashboardStyles.hero}>
        <div>
          <h1 className={dashboardStyles.title}>Wallets</h1>
          <p className={dashboardStyles.subtitle}>
            Startup wallets from this localnet, ready for TON Connect
          </p>
        </div>
      </section>

      <section className={styles.walletLayout}>
        <div className={styles.mainColumn}>
          <section
            className={`${styles.walletTableWrap} ${styles.walletsTableWrap}`}
            aria-labelledby="wallets-table-title"
          >
            <div className={styles.walletTableTitleBar}>
              <h2 id="wallets-table-title" className={styles.walletTableTitle}>
                Startup wallets
              </h2>
              <Button
                type="button"
                size="sm"
                className={styles.refreshButton}
                onClick={() => void handleRefreshWallets()}
                disabled={
                  runtimeWallets.length === 0 || isRefreshingBalances || walletTokensLoading
                }
              >
                <RefreshCw
                  size={14}
                  className={isRefreshingBalances || walletTokensLoading ? styles.spinning : ""}
                />
                Refresh
              </Button>
            </div>

            {isBusy ? (
              <WalletRowsSkeleton />
            ) : runtimeWallets.length === 0 ? (
              <div className={`${dashboardStyles.emptyState} ${styles.walletTableEmpty}`}>
                No supported startup wallets, start localnet with `--accounts` or
                `[localnet].accounts`
              </div>
            ) : (
              <table className={`${styles.walletTable} ${styles.walletsTable}`}>
                <thead>
                  <tr>
                    <th className={styles.walletNameHeader}>Name</th>
                    <th className={styles.walletAddressHeader}>Address</th>
                    <th className={styles.walletVersionHeader}>Version</th>
                    <th className={styles.walletBalanceHeader}>Balance</th>
                  </tr>
                </thead>
                <tbody>
                  {runtimeWallets.map(wallet => {
                    const balanceState = walletBalances[wallet.id]
                    const walletAddress = normalizeAddress(wallet.record.address, addressFormat)

                    return (
                      <tr key={wallet.id} className={styles.walletTableRow}>
                        <td className={styles.walletNameCell}>
                          <span className={styles.walletName} title={wallet.record.name}>
                            {wallet.record.name}
                          </span>
                        </td>
                        <td className={styles.walletAddressCell}>
                          <AddressChip
                            address={walletAddress}
                            fallback="Account"
                            copiedAddress={copiedAddress}
                            resolveName={false}
                            onAddressClick={(nextAddress, event) =>
                              openPath(`/explorer/address/${nextAddress}`, event)
                            }
                            onCopyAddress={handleCopyAddress}
                          />
                        </td>
                        <td className={styles.walletVersionCell}>
                          <span className={styles.walletVersion}>
                            {wallet.record.version.toUpperCase()}
                          </span>
                        </td>
                        <td className={styles.walletBalanceCell}>
                          <div className={styles.walletBalanceGroup}>
                            <WalletTokenPreview
                              address={walletAddress}
                              tokens={walletTokensById[wallet.id] ?? []}
                              loading={walletTokensLoading}
                              onOpenTokens={(address, event) =>
                                openPath(`/explorer/address/${address}#tokens`, event)
                              }
                            />
                            <span className={styles.walletBalance}>
                              {formatWalletBalanceLabel(balanceState)}
                            </span>
                          </div>
                        </td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
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
          </section>

          <section className={styles.walletTableWrap} aria-labelledby="wallet-sessions-title">
            <div className={styles.walletTableTitleBar}>
              <h2 id="wallet-sessions-title" className={styles.walletTableTitle}>
                Sessions
              </h2>
              <span className={styles.walletTableTitleMeta}>
                {pendingRequestCount === 0
                  ? "No pending approvals"
                  : `${pendingRequestCount} pending approval${pendingRequestCount === 1 ? "" : "s"}`}
              </span>
            </div>
            <table className={`${styles.walletTable} ${styles.sessionsTable}`}>
              <thead>
                <tr>
                  <th>dApp</th>
                  <th>Wallet</th>
                  <th className={styles.sessionActivityHeader}>Last activity</th>
                  <th className={styles.sessionActionsHeader} aria-label="Actions" />
                </tr>
              </thead>
              <tbody>
                {sessions.length === 0 ? (
                  <tr>
                    <td className={styles.sessionEmptyCell} colSpan={4}>
                      <div className={`${dashboardStyles.emptyState} ${styles.walletTableEmpty}`}>
                        No active TON Connect sessions
                      </div>
                    </td>
                  </tr>
                ) : (
                  sessions.map(session => (
                    <tr key={session.sessionId} className={styles.walletTableRow}>
                      <td className={styles.sessionDappCell}>
                        <span className={styles.sessionDappLine}>
                          <span className={styles.sessionTitle}>
                            {getDappName(session.dAppName)}
                          </span>
                          <span className={styles.sessionDappSeparator}>·</span>
                          <span className={styles.sessionDomain}>{session.domain}</span>
                        </span>
                      </td>
                      <td className={styles.sessionWalletCell}>
                        <SessionWalletCell
                          wallets={runtimeWallets}
                          walletId={session.walletId}
                          copiedAddress={copiedAddress}
                          addressFormat={addressFormat}
                          onAddressClick={(nextAddress, event) =>
                            openPath(`/explorer/address/${nextAddress}`, event)
                          }
                          onCopyAddress={handleCopyAddress}
                        />
                      </td>
                      <td className={styles.sessionActivityCell}>
                        {formatDateTime(session.lastActivityAt)}
                      </td>
                      <td className={styles.sessionActionsCell}>
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          className={styles.tableActionButton}
                          onClick={() => void handleDisconnectSession(session.sessionId)}
                          disabled={isSubmitting}
                        >
                          <Unplug size={14} />
                          Disconnect
                        </Button>
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
              <tfoot>
                <tr>
                  <td className={styles.connectFooterCell} colSpan={4}>
                    <form
                      className={styles.connectControlForm}
                      onSubmit={event => void handleConnectUrlSubmit(event)}
                    >
                      <label className={styles.connectInlineLabel} htmlFor="ton-connect-url">
                        Connect URL
                      </label>
                      <input
                        id="ton-connect-url"
                        className={styles.connectInput}
                        value={tonConnectUrl}
                        onChange={event => setTonConnectUrl(event.target.value)}
                        placeholder="tonconnect://..."
                        disabled={runtimeWallets.length === 0 || isSubmitting}
                      />
                      <Button
                        type="submit"
                        variant="outline"
                        size="sm"
                        className={styles.tableActionButton}
                        disabled={
                          runtimeWallets.length === 0 ||
                          tonConnectUrl.trim().length === 0 ||
                          isSubmitting
                        }
                      >
                        <Link2 size={14} />
                        Handle request
                      </Button>
                    </form>
                  </td>
                </tr>
              </tfoot>
            </table>
          </section>
        </div>
      </section>
    </>
  )
}

function WalletRowsSkeleton(): JSX.Element {
  return (
    <table className={`${styles.walletTable} ${styles.walletsTable}`} aria-label="Loading wallets">
      <thead>
        <tr>
          <th className={styles.walletNameHeader}>Name</th>
          <th className={styles.walletAddressHeader}>Address</th>
          <th className={styles.walletVersionHeader}>Version</th>
          <th className={styles.walletBalanceHeader}>Balance</th>
        </tr>
      </thead>
      <tbody>
        {Array.from({length: 4}, (_, index) => (
          <tr key={`wallet-row-skeleton-${index}`} className={styles.walletTableRow}>
            <td className={styles.walletNameCell}>
              <span className={`${dashboardStyles.skeletonLine} ${styles.walletNameSkeleton}`} />
            </td>
            <td className={styles.walletAddressCell}>
              <span className={`${dashboardStyles.skeletonLine} ${styles.walletAddressSkeleton}`} />
            </td>
            <td className={styles.walletVersionCell}>
              <span className={`${dashboardStyles.skeletonLine} ${styles.walletVersionSkeleton}`} />
            </td>
            <td className={styles.walletBalanceCell}>
              <span className={`${dashboardStyles.skeletonLine} ${styles.walletBalanceSkeleton}`} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

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

interface WalletTokenPreviewProps {
  readonly address: string
  readonly tokens: readonly JettonWallet[]
  readonly loading: boolean
  readonly onOpenTokens: (address: string, event?: ExplorerNavigationClickEvent) => void
}

const WalletTokenPreview: FC<WalletTokenPreviewProps> = ({
  address,
  tokens,
  loading,
  onOpenTokens,
}) => {
  if (loading && tokens.length === 0) {
    return <span className={styles.walletTokenPreviewSkeleton} aria-label="Loading tokens" />
  }

  if (tokens.length === 0) {
    return null
  }

  const firstToken = tokens[0]
  const firstMaster = firstToken?.master
  const firstSymbol = firstMaster?.jetton_content.symbol || "tokens"
  const firstDecimals = parseJettonDecimals(firstMaster)
  const firstImageSources = getImageSources(firstMaster?.jetton_content, TOKEN_IMAGE_SOURCE_KEYS)
  const firstImage = getPrimaryImageSource(firstMaster?.jetton_content, TOKEN_IMAGE_SOURCE_KEYS)
  const previewTokens = tokens.slice(1, TOKEN_PREVIEW_LIMIT)

  return (
    <button
      type="button"
      className={styles.walletTokenPreviewButton}
      onClick={event => onOpenTokens(address, event)}
      title="Open wallet tokens"
      aria-label="Open wallet tokens"
    >
      <img
        src={firstImage}
        alt=""
        className={styles.walletTokenPreviewIcon}
        onError={event => replaceBrokenImageWithFallback(event, firstImageSources)}
      />
      <span className={styles.walletTokenPreviewAmount}>
        {formatTokenAmount(firstToken.balance, firstDecimals)} {firstSymbol}
      </span>
      {previewTokens.length > 0 && (
        <span className={styles.walletTokenPreviewStack} aria-hidden="true">
          {previewTokens.map((token, index) => {
            const imageSources = getImageSources(
              token.master?.jetton_content,
              TOKEN_IMAGE_SOURCE_KEYS,
            )
            const image = imageSources[0]
            return image ? (
              <img
                key={token.address}
                src={image}
                alt=""
                className={styles.walletTokenPreviewStackIcon}
                style={{zIndex: previewTokens.length - index}}
                onError={event => replaceBrokenImageWithFallback(event, imageSources)}
              />
            ) : (
              <span
                key={token.address}
                className={styles.walletTokenPreviewStackPlaceholder}
                style={{zIndex: previewTokens.length - index}}
              />
            )
          })}
        </span>
      )}
      <span className={styles.walletTokenPreviewAction}>View all</span>
    </button>
  )
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

interface SessionWalletCellProps {
  readonly wallets: readonly RuntimeWallet[]
  readonly walletId: string
  readonly copiedAddress?: string
  readonly addressFormat: AddressFormatOptions
  readonly onAddressClick: (address: string, event?: ExplorerNavigationClickEvent) => void
  readonly onCopyAddress: (address: string) => Promise<void>
}

const SessionWalletCell: FC<SessionWalletCellProps> = ({
  wallets,
  walletId,
  copiedAddress,
  addressFormat,
  onAddressClick,
  onCopyAddress,
}) => {
  const wallet = findRuntimeWallet(wallets, walletId)
  if (!wallet) {
    return <span className={styles.sessionWalletFallback}>Unknown wallet</span>
  }

  const walletAddress = normalizeAddress(wallet.record.address, addressFormat)
  return (
    <AddressChip
      address={walletAddress}
      copiedAddress={copiedAddress}
      nameFallback={wallet.record.name}
      onAddressClick={onAddressClick}
      onCopyAddress={onCopyAddress}
    />
  )
}

function findRuntimeWallet(
  wallets: readonly RuntimeWallet[],
  walletId: string,
): RuntimeWallet | undefined {
  return wallets.find(wallet => wallet.id === walletId)
}

function sortJettonWalletsByAmount(wallets: readonly JettonWallet[]): JettonWallet[] {
  return [...wallets].sort(compareJettonWalletAmount)
}

function compareJettonWalletAmount(left: JettonWallet, right: JettonWallet): number {
  const leftAmount = normalizeJettonAmount(left)
  const rightAmount = normalizeJettonAmount(right)
  if (leftAmount === rightAmount) {
    return 0
  }
  return leftAmount > rightAmount ? -1 : 1
}

function normalizeJettonAmount(wallet: JettonWallet): number {
  const decimals = parseJettonDecimals(wallet.master)
  const amount = Number(wallet.balance) / 10 ** decimals
  return Number.isFinite(amount) ? amount : 0
}

function parseJettonDecimals(master: JettonMasterMetadata | undefined): number {
  const decimals = Number(master?.jetton_content.decimals)
  return Number.isFinite(decimals) ? decimals : 9
}

function formatTokenAmount(value: string, decimals: number): string {
  const decimalsNumber = Number.isFinite(decimals) ? decimals : 9
  return (Number(value) / 10 ** decimalsNumber).toLocaleString(undefined, {
    maximumFractionDigits: decimalsNumber,
  })
}

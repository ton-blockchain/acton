import type {FC, FormEvent} from "react"
import {useCallback, useEffect, useRef, useState} from "react"
import {Link2, RefreshCw, Unplug} from "lucide-react"
import {
  Button,
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
  DateTime,
  InlineButton,
  Input,
} from "@acton/ui"

import type {TonClient} from "@acton/explorer-core/api/client"
import {
  loadJettonWalletsWithMasters,
  sortJettonWalletsByAmount,
} from "@acton/explorer-core/api/jettonWallets"
import type {JettonWallet} from "@acton/explorer-core/api/types"
import {ExplorerAddressChip} from "@acton/explorer-core/components/ExplorerAddressChip"
import {WalletAccountSummary} from "@acton/explorer-core/components/WalletAccountSummary"
import {
  type AddressFormatOptions,
  normalizeAddress,
  toRawAddress,
} from "@acton/explorer-core/components/utils"
import {useAddressFormat} from "@acton/explorer-core/hooks/useNetworkInfo"
import {useExplorerRoutePaths} from "@acton/explorer-core/hooks/useExplorerRoutePaths"
import {
  type ExplorerNavigationClickEvent,
  useOpenExplorerPath,
} from "@acton/explorer-core/hooks/useOpenExplorerPath"
import type {RuntimeWallet} from "../../wallet/types"
import {useWalletRuntime} from "../../wallet/useWalletRuntime"

import styles from "./WalletsPage.module.css"

interface WalletsPageProps {
  readonly client: TonClient
}

type WalletTokensById = Readonly<Record<string, readonly JettonWallet[]>>

export const WalletsPage: FC<WalletsPageProps> = ({client}) => {
  const addressFormat = useAddressFormat()
  const routes = useExplorerRoutePaths()
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
        const tokenWallets = await loadJettonWalletsWithMasters(client, ownerAddresses)
        const nextTokensById: Record<string, JettonWallet[]> = {}
        for (const wallet of wallets) {
          nextTokensById[wallet.id] = []
        }
        for (const tokenWallet of tokenWallets) {
          const walletId = ownerByRawAddress.get(toRawAddress(tokenWallet.owner))
          if (!walletId) {
            continue
          }
          nextTokensById[walletId].push(tokenWallet)
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
      <section className={styles.walletLayout}>
        <div className={styles.mainColumn}>
          <DataTable
            title="Project wallets"
            titleId="wallets-table-title"
            minWidth="62rem"
            aria-labelledby="wallets-table-title"
            actions={
              <InlineButton
                variant="accent"
                leadingIcon={
                  <RefreshCw
                    size={14}
                    className={isRefreshingBalances || walletTokensLoading ? styles.spinning : ""}
                  />
                }
                onClick={() => void handleRefreshWallets()}
                disabled={
                  runtimeWallets.length === 0 || isRefreshingBalances || walletTokensLoading
                }
              >
                Refresh
              </InlineButton>
            }
          >
            <DataTableTable aria-label="Project wallets">
              <DataTableHead>
                <DataTableRow>
                  <DataTableHeaderCell columnWidth="16rem">Name</DataTableHeaderCell>
                  <DataTableHeaderCell columnWidth="21rem">Address</DataTableHeaderCell>
                  <DataTableHeaderCell columnWidth="8rem">Version</DataTableHeaderCell>
                  <DataTableHeaderCell align="right">Balance</DataTableHeaderCell>
                </DataTableRow>
              </DataTableHead>
              <DataTableBody>
                {isBusy ? (
                  <DataTableSkeletonRows
                    columns={4}
                    rows={4}
                    rowKeyPrefix="wallet-row-skeleton"
                    alignments={["left", "left", "left", "right"]}
                    widths={["10rem", "16rem", "3rem", "7rem"]}
                  />
                ) : runtimeWallets.length === 0 ? (
                  <DataTableEmpty colSpan={4}>
                    No supported project wallets are configured in Acton.toml
                  </DataTableEmpty>
                ) : (
                  runtimeWallets.map(wallet => {
                    const balanceState = walletBalances[wallet.id]
                    const walletAddress = normalizeAddress(wallet.record.address, addressFormat)

                    return (
                      <DataTableRow key={wallet.id} hover>
                        <DataTableCell tone="strong" truncate title={wallet.record.name}>
                          {wallet.record.name}
                        </DataTableCell>
                        <DataTableCell className={styles.walletAddressCell}>
                          <ExplorerAddressChip
                            address={walletAddress}
                            fallback="Account"
                            copiedAddress={copiedAddress}
                            resolveName={false}
                            onAddressClick={(nextAddress, event) =>
                              openPath(routes.addressPath(nextAddress), event)
                            }
                            onCopyAddress={handleCopyAddress}
                          />
                        </DataTableCell>
                        <DataTableCell tone="strong" truncate>
                          {wallet.record.version.toUpperCase()}
                        </DataTableCell>
                        <DataTableCell align="right" className={styles.walletBalanceCell}>
                          <WalletAccountSummary
                            address={walletAddress}
                            tokens={walletTokensById[wallet.id] ?? []}
                            tokensLoading={walletTokensLoading}
                            balanceState={balanceState}
                            onOpenTokens={(address, event) =>
                              openPath(`${routes.addressPath(address)}#tokens`, event)
                            }
                          />
                        </DataTableCell>
                      </DataTableRow>
                    )
                  })
                )}
              </DataTableBody>
            </DataTableTable>

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
          </DataTable>

          <DataTable
            title="Sessions"
            titleId="wallet-sessions-title"
            minWidth="56rem"
            aria-labelledby="wallet-sessions-title"
            meta={
              pendingRequestCount === 0
                ? "No pending approvals"
                : `${pendingRequestCount} pending approval${pendingRequestCount === 1 ? "" : "s"}`
            }
          >
            <DataTableTable aria-label="TON Connect sessions">
              <DataTableHead>
                <DataTableRow>
                  <DataTableHeaderCell>dApp</DataTableHeaderCell>
                  <DataTableHeaderCell columnWidth="18rem">Wallet</DataTableHeaderCell>
                  <DataTableHeaderCell columnWidth="15rem">Last activity</DataTableHeaderCell>
                  <DataTableHeaderCell align="right" columnWidth="10rem" aria-label="Actions" />
                </DataTableRow>
              </DataTableHead>
              <DataTableBody>
                {sessions.length === 0 ? (
                  <DataTableEmpty colSpan={4}>No active TON Connect sessions</DataTableEmpty>
                ) : (
                  sessions.map(session => (
                    <DataTableRow key={session.sessionId} hover>
                      <DataTableCell className={styles.sessionDappCell}>
                        <span className={styles.sessionDappLine}>
                          <span className={styles.sessionTitle}>
                            {getDappName(session.dAppName)}
                          </span>
                          <span className={styles.sessionDappSeparator}>·</span>
                          <span className={styles.sessionDomain}>{session.domain}</span>
                        </span>
                      </DataTableCell>
                      <DataTableCell className={styles.sessionWalletCell}>
                        <SessionWalletCell
                          wallets={runtimeWallets}
                          walletId={session.walletId}
                          copiedAddress={copiedAddress}
                          addressFormat={addressFormat}
                          onAddressClick={(nextAddress, event) =>
                            openPath(routes.addressPath(nextAddress), event)
                          }
                          onCopyAddress={handleCopyAddress}
                        />
                      </DataTableCell>
                      <DataTableCell tone="muted" truncate>
                        <DateTime
                          fallback={session.lastActivityAt}
                          value={session.lastActivityAt}
                        />
                      </DataTableCell>
                      <DataTableCell align="right">
                        <InlineButton
                          variant="danger"
                          leadingIcon={<Unplug size={14} />}
                          onClick={() => void handleDisconnectSession(session.sessionId)}
                          disabled={isSubmitting}
                        >
                          Disconnect
                        </InlineButton>
                      </DataTableCell>
                    </DataTableRow>
                  ))
                )}
              </DataTableBody>
              <DataTableFooter>
                <DataTableRow>
                  <DataTableCell className={styles.connectFooterCell} colSpan={4}>
                    <form
                      className={styles.connectControlForm}
                      onSubmit={event => void handleConnectUrlSubmit(event)}
                    >
                      <label className={styles.connectInlineLabel} htmlFor="ton-connect-url">
                        Connect URL
                      </label>
                      <Input
                        size="sm"
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
                        className={styles.connectAction}
                        leadingIcon={<Link2 size={14} />}
                        disabled={
                          runtimeWallets.length === 0 ||
                          tonConnectUrl.trim().length === 0 ||
                          isSubmitting
                        }
                      >
                        Handle request
                      </Button>
                    </form>
                  </DataTableCell>
                </DataTableRow>
              </DataTableFooter>
            </DataTableTable>
          </DataTable>
        </div>
      </section>
    </>
  )
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
  const wallet = wallets.find(wallet => wallet.id === walletId)
  if (!wallet) {
    return <span className={styles.sessionWalletFallback}>Unknown wallet</span>
  }

  const walletAddress = normalizeAddress(wallet.record.address, addressFormat)
  return (
    <ExplorerAddressChip
      address={walletAddress}
      copiedAddress={copiedAddress}
      nameFallback={wallet.record.name}
      onAddressClick={onAddressClick}
      onCopyAddress={onCopyAddress}
    />
  )
}

import {useEffect, useRef, useState} from "react"
import type {ChangeEvent, FC} from "react"
import {Link, useNavigate} from "react-router"
import {
  BlockChip,
  Button,
  Checkbox,
  DateTime,
  Dialog,
  DialogActions,
  EmptyState,
  InlineAction,
  InlineActions,
  Pagination,
  shortenMiddle,
  useClientPagination,
  useToast,
} from "@acton/ui"
import {Download, Info, Star, Trash2, TriangleAlert, Upload} from "lucide-react"

import type {TonClient} from "../api/client"
import {loadJettonWalletsWithMasters, sortJettonWalletsForDisplay} from "../api/jettonWallets"
import type {JettonWallet} from "../api/types"
import {ExplorerAddressChip} from "../components/ExplorerAddressChip"
import {ExplorerBreadcrumbs} from "../components/ExplorerBreadcrumbs"
import {WalletAccountSummary, type AccountBalanceState} from "../components/WalletAccountSummary"
import {normalizeAddress, toRawAddress} from "../components/utils"
import {
  createFavoritesBundle,
  parseFavoritesBundle,
  type FavoritesBundle,
} from "../hooks/favoritesBundle"
import {useAddressBook} from "../hooks/useAddressBook"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useFavoriteAccounts, type FavoriteAccount} from "../hooks/useFavoriteAccounts"
import {useFavoriteBlocks, type FavoriteBlock} from "../hooks/useFavoriteBlocks"
import {useFavoriteTransactions, type FavoriteTransaction} from "../hooks/useFavoriteTransactions"
import {useAddressFormat, useNetworkInfo} from "../hooks/useNetworkInfo"
import {useOpenExplorerPath} from "../hooks/useOpenExplorerPath"

import styles from "./FavoriteAccountsPage.module.css"

interface FavoriteAccountsPageProps {
  readonly client: TonClient
}

type BalancesByAddress = Readonly<Record<string, AccountBalanceState>>
type TokensByAddress = Readonly<Record<string, readonly JettonWallet[]>>
type BundleSection = "accounts" | "blocks" | "transactions" | "addressNames"
type BundleSelection = Readonly<Record<BundleSection, boolean>>

const BUNDLE_SECTIONS: readonly {
  readonly key: BundleSection
  readonly label: string
  readonly description: string
}[] = [
  {key: "accounts", label: "Accounts", description: "Favorite account addresses and saved times"},
  {key: "blocks", label: "Blocks", description: "Favorite block references and saved times"},
  {
    key: "transactions",
    label: "Transactions",
    description: "Favorite transaction hashes and saved times",
  },
  {
    key: "addressNames",
    label: "Local contract names",
    description: "Names saved in this browser for contract addresses",
  },
]

const EMPTY_BUNDLE_SELECTION: BundleSelection = {
  accounts: false,
  blocks: false,
  transactions: false,
  addressNames: false,
}

export const FavoriteAccountsPage: FC<FavoriteAccountsPageProps> = ({client}) => {
  const routes = useExplorerRoutePaths()
  const navigate = useNavigate()
  const addressFormat = useAddressFormat()
  const {network} = useNetworkInfo()
  const openPath = useOpenExplorerPath()
  const {favorites, importFavorites: importAccountFavorites, setFavorite} = useFavoriteAccounts()
  const {
    favorites: favoriteBlocks,
    importFavorites: importBlockFavorites,
    setFavorite: setFavoriteBlock,
  } = useFavoriteBlocks()
  const {
    favorites: favoriteTransactions,
    importFavorites: importTransactionFavorites,
    setFavorite: setFavoriteTransaction,
  } = useFavoriteTransactions()
  const {localAddressNames, prefetchNames, setAddressName} = useAddressBook()
  const {showToast} = useToast()
  const [balancesByAddress, setBalancesByAddress] = useState<BalancesByAddress>({})
  const [tokensByAddress, setTokensByAddress] = useState<TokensByAddress>({})
  const [tokensLoading, setTokensLoading] = useState(false)
  const [importDialogOpen, setImportDialogOpen] = useState(false)
  const [importBundle, setImportBundle] = useState<FavoritesBundle>()
  const [importSelection, setImportSelection] = useState<BundleSelection>(EMPTY_BUNDLE_SELECTION)
  const [importing, setImporting] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const accountDataRequestRef = useRef(0)
  const accountPagination = useClientPagination(favorites)
  const blockPagination = useClientPagination(favoriteBlocks)
  const transactionPagination = useClientPagination(favoriteTransactions)

  useEffect(() => {
    void prefetchNames(accountPagination.currentItems.map(favorite => favorite.address))
  }, [accountPagination.currentItems, prefetchNames])

  useEffect(() => {
    const requestId = accountDataRequestRef.current + 1
    accountDataRequestRef.current = requestId

    if (accountPagination.currentItems.length === 0) {
      setBalancesByAddress({})
      setTokensByAddress({})
      setTokensLoading(false)
      return
    }

    const ownerByRawAddress = new Map<string, string>()
    const ownerAddresses = accountPagination.currentItems.map(favorite => {
      const address = normalizeAddress(favorite.address, addressFormat)
      ownerByRawAddress.set(toRawAddress(address), favorite.address)
      return address
    })

    setBalancesByAddress(current => {
      const nextBalances: Record<string, AccountBalanceState> = {}
      for (const favorite of accountPagination.currentItems) {
        const previousBalance = current[favorite.address]
        nextBalances[favorite.address] = previousBalance?.value
          ? {...previousBalance, isLoading: true, error: undefined}
          : {isLoading: true}
      }
      return nextBalances
    })
    setTokensByAddress(current => {
      const nextTokens: Record<string, readonly JettonWallet[]> = {}
      for (const favorite of accountPagination.currentItems) {
        nextTokens[favorite.address] = current[favorite.address] ?? []
      }
      return nextTokens
    })
    setTokensLoading(true)

    const loadFavoriteAccountData = async () => {
      const [accountStatesResult, tokenWalletsResult] = await Promise.allSettled([
        client.getAccountStates(ownerAddresses, false),
        loadJettonWalletsWithMasters(client, ownerAddresses),
      ])

      if (accountDataRequestRef.current !== requestId) {
        return
      }

      if (accountStatesResult.status === "fulfilled") {
        const accountsByRawAddress = new Map(
          accountStatesResult.value.accounts.map(account => [
            toRawAddress(account.address),
            account,
          ]),
        )
        const nextBalances: Record<string, AccountBalanceState> = {}
        for (const favorite of accountPagination.currentItems) {
          const account = accountsByRawAddress.get(
            toRawAddress(normalizeAddress(favorite.address, addressFormat)),
          )
          nextBalances[favorite.address] = account
            ? {value: account.balance, isLoading: false}
            : {isLoading: false, error: "Account state not found"}
        }
        setBalancesByAddress(nextBalances)
      } else {
        console.error("Failed to fetch favorite account balances", accountStatesResult.reason)
        const nextBalances: Record<string, AccountBalanceState> = {}
        for (const favorite of accountPagination.currentItems) {
          nextBalances[favorite.address] = {isLoading: false, error: "Balance unavailable"}
        }
        setBalancesByAddress(nextBalances)
      }

      if (tokenWalletsResult.status === "fulfilled") {
        const nextTokensByAddress: Record<string, JettonWallet[]> = {}
        for (const favorite of accountPagination.currentItems) {
          nextTokensByAddress[favorite.address] = []
        }
        for (const tokenWallet of tokenWalletsResult.value) {
          const ownerAddress = ownerByRawAddress.get(toRawAddress(tokenWallet.owner))
          if (ownerAddress) {
            nextTokensByAddress[ownerAddress].push(tokenWallet)
          }
        }
        for (const [address, tokenWallets] of Object.entries(nextTokensByAddress)) {
          nextTokensByAddress[address] = sortJettonWalletsForDisplay(tokenWallets)
        }
        setTokensByAddress(nextTokensByAddress)
      } else {
        console.error("Failed to fetch favorite account token balances", tokenWalletsResult.reason)
        setTokensByAddress({})
      }
      setTokensLoading(false)
    }

    void loadFavoriteAccountData()
  }, [accountPagination.currentItems, addressFormat, client])

  const handleRemoveAccount = (favorite: FavoriteAccount) => {
    setFavorite(favorite.address, false)
    showToast({
      title: "Account removed from favorites",
      variant: "success",
    })
  }

  const handleRemoveBlock = (favorite: FavoriteBlock) => {
    setFavoriteBlock(favorite, false)
    showToast({
      title: "Block removed from favorites",
      variant: "success",
    })
  }

  const handleRemoveTransaction = (favorite: FavoriteTransaction) => {
    setFavoriteTransaction(favorite, false)
    showToast({
      title: "Transaction removed from favorites",
      variant: "success",
    })
  }

  const handleExport = () => {
    const bundle = createFavoritesBundle({
      network: network.id,
      accounts: favorites,
      blocks: favoriteBlocks,
      transactions: favoriteTransactions,
      addressNames: localAddressNames,
    })
    const blob = new Blob([JSON.stringify(bundle, null, 2)], {type: "application/json"})
    const url = URL.createObjectURL(blob)
    const link = document.createElement("a")
    link.href = url
    link.download = `acton-favorites-${network.id}-${formatFilenameDate(new Date())}.json`
    document.body.append(link)
    link.click()
    link.remove()
    URL.revokeObjectURL(url)
    showToast({title: "Favorites exported", variant: "success"})
  }

  const handleImportFile = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    event.target.value = ""
    if (!file) return

    try {
      if (file.size > 5 * 1024 * 1024) {
        throw new Error("The selected file is larger than 5 MB")
      }
      const bundle = parseFavoritesBundle(await file.text())
      const selection = getBundleSelection(bundle)
      if (!hasSelectedBundleData(selection)) {
        throw new Error("The bundle does not contain importable data")
      }
      setImportBundle(bundle)
      setImportSelection(selection)
      setImportDialogOpen(true)
    } catch (error) {
      showToast({
        title: "Import failed",
        description: error instanceof Error ? error.message : "Could not read the favorites bundle",
        variant: "error",
      })
    }
  }

  const handleImport = async () => {
    if (importing || !importBundle || !hasSelectedBundleData(importSelection)) return

    setImporting(true)
    try {
      if (importSelection.accounts) {
        importAccountFavorites(importBundle.accounts)
      }
      if (importSelection.blocks) {
        importBlockFavorites(importBundle.blocks)
      }
      if (importSelection.transactions) {
        importTransactionFavorites(importBundle.transactions)
      }
      if (importSelection.addressNames) {
        for (const entry of importBundle.addressNames) {
          await setAddressName(entry.address, entry.name)
        }
      }

      setImportDialogOpen(false)
      setImportBundle(undefined)
      setImportSelection(EMPTY_BUNDLE_SELECTION)
      showToast({title: "Favorites imported", variant: "success"})
    } catch (error) {
      showToast({
        title: "Import failed",
        description:
          error instanceof Error ? error.message : "Could not import the favorites bundle",
        variant: "error",
      })
    } finally {
      setImporting(false)
    }
  }

  const handleImportDialogChange = (open: boolean) => {
    if (importing && !open) return
    setImportDialogOpen(open)
    if (!open) {
      setImportBundle(undefined)
      setImportSelection(EMPTY_BUNDLE_SELECTION)
    }
  }

  const hasFavorites =
    favorites.length > 0 || favoriteBlocks.length > 0 || favoriteTransactions.length > 0

  return (
    <section className={styles.container}>
      <ExplorerBreadcrumbs items={[{label: "Favorites"}]} />
      <header className={styles.hero}>
        <div>
          <h1 className={styles.title}>Favorites</h1>
        </div>
        <div className={styles.heroActions}>
          <input
            ref={fileInputRef}
            className={styles.hiddenFileInput}
            type="file"
            accept="application/json,.json"
            aria-label="Import favorites JSON"
            onChange={event => void handleImportFile(event)}
          />
          <Button
            size="sm"
            variant="primary"
            leadingIcon={<Upload size={15} />}
            onClick={() => fileInputRef.current?.click()}
          >
            Import JSON
          </Button>
          <Button
            size="sm"
            variant="outline"
            leadingIcon={<Download size={15} />}
            onClick={handleExport}
          >
            Export JSON
          </Button>
        </div>
      </header>

      {!hasFavorites && (
        <section className={styles.tableFrame}>
          <header className={styles.tableTitle}>
            <Star size={16} className={styles.titleIcon} />
            <span>Favorites</span>
          </header>
          <EmptyState
            icon={<Star size={20} aria-hidden="true" />}
            title="No favorites yet"
            description="Use the star on an account, block, or transaction page to save it here"
            action={
              <Button size="sm" variant="primary" onClick={() => void navigate(routes.rootPath)}>
                Explore TON
              </Button>
            }
          />
        </section>
      )}

      {favorites.length > 0 && (
        <section className={styles.tableFrame} aria-label="Favorite accounts">
          <header className={styles.tableTitle}>
            <Star size={16} className={styles.titleIcon} />
            <span>Accounts</span>
          </header>
          <div className={styles.tableScroller}>
            <table className={styles.table}>
              <thead>
                <tr>
                  <th className={styles.accountHeader}>Account</th>
                  <th className={styles.balanceHeader}>Balance</th>
                  <th className={styles.savedAtHeader}>Saved at</th>
                </tr>
              </thead>
              <tbody>
                {accountPagination.currentItems.map(favorite => (
                  <tr key={favorite.address} className={styles.tableRow}>
                    <td className={styles.accountCell}>
                      <InlineActions
                        visibility="always"
                        actions={
                          <InlineAction
                            label="Remove from favorites"
                            icon={<Trash2 />}
                            onClick={() => handleRemoveAccount(favorite)}
                          />
                        }
                      >
                        <ExplorerAddressChip
                          address={favorite.address}
                          fallback="Account"
                          copyable={false}
                          onAddressClick={(address, event) =>
                            openPath(routes.addressPath(address), event)
                          }
                        />
                      </InlineActions>
                    </td>
                    <td className={styles.balanceCell}>
                      <WalletAccountSummary
                        address={favorite.address}
                        tokens={tokensByAddress[favorite.address] ?? []}
                        tokensLoading={tokensLoading}
                        balanceState={balancesByAddress[favorite.address]}
                        onOpenTokens={(address, event) =>
                          openPath(`${routes.addressPath(address)}#tokens`, event)
                        }
                      />
                    </td>
                    <td className={styles.savedAtCell}>
                      <DateTime value={positiveTime(favorite.savedAt)} fallback="Unknown" />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <Pagination
            currentPage={accountPagination.currentPage}
            totalItems={accountPagination.totalItems}
            pageSize={accountPagination.pageSize}
            onPageChange={accountPagination.setCurrentPage}
            label="Favorite accounts pagination"
          />
        </section>
      )}

      {favoriteBlocks.length > 0 && (
        <section className={styles.tableFrame} aria-label="Favorite blocks">
          <header className={styles.tableTitle}>
            <Star size={16} className={styles.titleIcon} />
            <span>Blocks</span>
          </header>
          <div className={styles.tableScroller}>
            <table className={`${styles.table} ${styles.blockTable}`}>
              <thead>
                <tr>
                  <th className={styles.blockHeader}>Block</th>
                  <th className={styles.blockTimeHeader}>Generated at</th>
                  <th className={styles.blockTimeHeader}>Saved at</th>
                </tr>
              </thead>
              <tbody>
                {blockPagination.currentItems.map(favorite => {
                  const path = routes.blockPath(favorite.workchain, favorite.shard, favorite.seqno)
                  return (
                    <tr
                      key={`${favorite.workchain}:${favorite.shard}:${favorite.seqno}`}
                      className={styles.tableRow}
                    >
                      <td className={styles.blockCell}>
                        <div className={styles.blockCellContent}>
                          <BlockChip
                            workchain={favorite.workchain}
                            shard={favorite.shard}
                            seqno={favorite.seqno}
                            copyable={false}
                            display="full"
                            href={path}
                            onClick={event => openPath(path, event)}
                          />
                          <InlineAction
                            label="Remove from favorites"
                            icon={<Trash2 />}
                            onClick={() => handleRemoveBlock(favorite)}
                          />
                        </div>
                      </td>
                      <td className={styles.blockTimeCell}>
                        <DateTime
                          fallback="Unknown"
                          unit="seconds"
                          value={positiveTime(favorite.generatedAt)}
                        />
                      </td>
                      <td className={styles.blockTimeCell}>
                        <DateTime value={positiveTime(favorite.savedAt)} fallback="Unknown" />
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>
          <Pagination
            currentPage={blockPagination.currentPage}
            totalItems={blockPagination.totalItems}
            pageSize={blockPagination.pageSize}
            onPageChange={blockPagination.setCurrentPage}
            label="Favorite blocks pagination"
          />
        </section>
      )}

      {favoriteTransactions.length > 0 && (
        <section className={styles.tableFrame} aria-label="Favorite transactions">
          <header className={styles.tableTitle}>
            <Star size={16} className={styles.titleIcon} />
            <span>Transactions</span>
          </header>
          <div className={styles.tableScroller}>
            <table className={`${styles.table} ${styles.transactionTable}`}>
              <thead>
                <tr>
                  <th className={styles.hashHeader}>Transaction</th>
                  <th className={styles.transactionAccountHeader}>Account</th>
                  <th className={styles.ltHeader}>Logical time</th>
                  <th className={styles.savedAtHeader}>Saved at</th>
                </tr>
              </thead>
              <tbody>
                {transactionPagination.currentItems.map(favorite => (
                  <tr key={favorite.hash} className={styles.tableRow}>
                    <td className={styles.hashCell}>
                      <InlineActions
                        visibility="always"
                        actions={
                          <InlineAction
                            label="Remove from favorites"
                            icon={<Trash2 />}
                            onClick={() => handleRemoveTransaction(favorite)}
                          />
                        }
                      >
                        <Link
                          className={styles.transactionLink}
                          to={routes.transactionPath(favorite.hash)}
                          title={favorite.hash}
                        >
                          {shortenMiddle(favorite.hash, {start: 6, end: 6})}
                        </Link>
                      </InlineActions>
                    </td>
                    <td className={styles.transactionAccountCell}>
                      {favorite.account ? (
                        <ExplorerAddressChip
                          address={favorite.account}
                          fallback="Account"
                          copyable={false}
                          onAddressClick={(address, event) =>
                            openPath(routes.addressPath(address), event)
                          }
                        />
                      ) : (
                        <span className={styles.missingValue}>Unknown</span>
                      )}
                    </td>
                    <td className={styles.ltCell}>{favorite.lt ?? "—"}</td>
                    <td className={styles.savedAtCell}>
                      <DateTime value={positiveTime(favorite.savedAt)} fallback="Unknown" />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <Pagination
            currentPage={transactionPagination.currentPage}
            totalItems={transactionPagination.totalItems}
            pageSize={transactionPagination.pageSize}
            onPageChange={transactionPagination.setCurrentPage}
            label="Favorite transactions pagination"
          />
        </section>
      )}

      <Dialog
        open={importDialogOpen}
        onOpenChange={handleImportDialogChange}
        busy={importing}
        title="Import favorites"
        description="Choose which sections to add to this browser"
        leadingIcon={<Upload size={18} />}
        maxWidth={560}
        contentClassName={styles.importDialogContent}
      >
        {importBundle && (
          <>
            <div className={styles.importNetworkRow}>
              <span className={styles.importNetworkLabel}>Network</span>
              <strong className={styles.importNetworkBadge}>
                {formatBundleNetwork(importBundle.network)}
              </strong>
            </div>
            {importBundle.network !== network.id && (
              <div className={styles.importWarning}>
                <TriangleAlert size={17} aria-hidden="true" />
                <span>
                  This bundle was exported from {formatBundleNetwork(importBundle.network)}.
                  Selected data will be added to {formatBundleNetwork(network.id)}
                </span>
              </div>
            )}
            <div className={styles.importSections}>
              {BUNDLE_SECTIONS.map(section => {
                const count = importBundle[section.key].length
                return (
                  <Checkbox
                    key={section.key}
                    className={styles.importSection}
                    checked={importSelection[section.key]}
                    disabled={count === 0}
                    label={section.label}
                    count={count}
                    description={section.description}
                    onChange={event =>
                      setImportSelection(current => ({
                        ...current,
                        [section.key]: event.target.checked,
                      }))
                    }
                  />
                )
              })}
            </div>
            <div className={styles.importSummaryHint}>
              <Info size={14} aria-hidden="true" />
              <span>Selected sections will be merged with existing data</span>
            </div>
            <DialogActions className={styles.importActions}>
              <Button
                variant="outline"
                disabled={importing}
                onClick={() => handleImportDialogChange(false)}
              >
                Cancel
              </Button>
              <Button
                variant="primary"
                disabled={!hasSelectedBundleData(importSelection)}
                loading={importing}
                onClick={() => void handleImport()}
              >
                Import selected
              </Button>
            </DialogActions>
          </>
        )}
      </Dialog>
    </section>
  )
}

function positiveTime(value: number | undefined): number | undefined {
  return value !== undefined && Number.isFinite(value) && value > 0 ? value : undefined
}

function getBundleSelection(bundle: FavoritesBundle): BundleSelection {
  return {
    accounts: bundle.accounts.length > 0,
    blocks: bundle.blocks.length > 0,
    transactions: bundle.transactions.length > 0,
    addressNames: bundle.addressNames.length > 0,
  }
}

function hasSelectedBundleData(selection: BundleSelection): boolean {
  return Object.values(selection).some(Boolean)
}

function formatBundleNetwork(network: string): string {
  return network ? `${network[0].toUpperCase()}${network.slice(1)}` : network
}

function formatFilenameDate(date: Date): string {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, "0")
  const day = String(date.getDate()).padStart(2, "0")
  return `${year}-${month}-${day}`
}

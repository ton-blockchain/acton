import {useEffect, useRef, useState} from "react"
import type {FC} from "react"
import {Link} from "react-router-dom"
import {useToast} from "@acton/shared-ui"
import {Star, Trash2} from "lucide-react"

import type {TonClient} from "../api/client"
import {loadJettonWalletsWithMasters, sortJettonWalletsByAmount} from "../api/jettonWallets"
import type {JettonWallet} from "../api/types"
import {AddressChip} from "../components/AddressChip"
import {Breadcrumbs} from "../components/Breadcrumbs"
import {InlineActionButton, InlineActionGroup} from "../components/InlineActionButton"
import {WalletAccountSummary, type AccountBalanceState} from "../components/WalletAccountSummary"
import {normalizeAddress, toRawAddress} from "../components/utils"
import {useAddressBook} from "../hooks/useAddressBook"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useFavoriteAccounts, type FavoriteAccount} from "../hooks/useFavoriteAccounts"
import {useAddressFormat} from "../hooks/useNetworkInfo"
import {useOpenExplorerPath} from "../hooks/useOpenExplorerPath"

import styles from "./FavoriteAccountsPage.module.css"

interface FavoriteAccountsPageProps {
  readonly client: TonClient
}

type BalancesByAddress = Readonly<Record<string, AccountBalanceState>>
type TokensByAddress = Readonly<Record<string, readonly JettonWallet[]>>

export const FavoriteAccountsPage: FC<FavoriteAccountsPageProps> = ({client}) => {
  const routes = useExplorerRoutePaths()
  const addressFormat = useAddressFormat()
  const openPath = useOpenExplorerPath()
  const {favorites, setFavorite} = useFavoriteAccounts()
  const {prefetchNames} = useAddressBook()
  const {showToast} = useToast()
  const [balancesByAddress, setBalancesByAddress] = useState<BalancesByAddress>({})
  const [tokensByAddress, setTokensByAddress] = useState<TokensByAddress>({})
  const [tokensLoading, setTokensLoading] = useState(false)
  const accountDataRequestRef = useRef(0)

  useEffect(() => {
    void prefetchNames(favorites.map(favorite => favorite.address))
  }, [favorites, prefetchNames])

  useEffect(() => {
    const requestId = accountDataRequestRef.current + 1
    accountDataRequestRef.current = requestId

    if (favorites.length === 0) {
      setBalancesByAddress({})
      setTokensByAddress({})
      setTokensLoading(false)
      return
    }

    const ownerByRawAddress = new Map<string, string>()
    const ownerAddresses = favorites.map(favorite => {
      const address = normalizeAddress(favorite.address, addressFormat)
      ownerByRawAddress.set(toRawAddress(address), favorite.address)
      return address
    })

    setBalancesByAddress(current => {
      const nextBalances: Record<string, AccountBalanceState> = {}
      for (const favorite of favorites) {
        const previousBalance = current[favorite.address]
        nextBalances[favorite.address] = previousBalance?.value
          ? {...previousBalance, isLoading: true, error: undefined}
          : {isLoading: true}
      }
      return nextBalances
    })
    setTokensByAddress(current => {
      const nextTokens: Record<string, readonly JettonWallet[]> = {}
      for (const favorite of favorites) {
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
        for (const favorite of favorites) {
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
        for (const favorite of favorites) {
          nextBalances[favorite.address] = {isLoading: false, error: "Balance unavailable"}
        }
        setBalancesByAddress(nextBalances)
      }

      if (tokenWalletsResult.status === "fulfilled") {
        const nextTokensByAddress: Record<string, JettonWallet[]> = {}
        for (const favorite of favorites) {
          nextTokensByAddress[favorite.address] = []
        }
        for (const tokenWallet of tokenWalletsResult.value) {
          const ownerAddress = ownerByRawAddress.get(toRawAddress(tokenWallet.owner))
          if (ownerAddress) {
            nextTokensByAddress[ownerAddress].push(tokenWallet)
          }
        }
        for (const [address, tokenWallets] of Object.entries(nextTokensByAddress)) {
          nextTokensByAddress[address] = sortJettonWalletsByAmount(tokenWallets)
        }
        setTokensByAddress(nextTokensByAddress)
      } else {
        console.error("Failed to fetch favorite account token balances", tokenWalletsResult.reason)
        setTokensByAddress({})
      }
      setTokensLoading(false)
    }

    void loadFavoriteAccountData()
  }, [addressFormat, client, favorites])

  const handleRemove = (favorite: FavoriteAccount) => {
    setFavorite(favorite.address, false)
    showToast({
      description: "Account removed from favorites",
      variant: "success",
    })
  }

  return (
    <section className={styles.container}>
      <Breadcrumbs items={[{label: "Favorite accounts"}]} />
      <header className={styles.hero}>
        <div>
          <h1 className={styles.title}>Favorite accounts</h1>
        </div>
      </header>

      <section className={styles.tableFrame}>
        <header className={styles.tableTitle}>
          <Star size={16} className={styles.titleIcon} />
          <span>Favorites</span>
        </header>
        {favorites.length === 0 ? (
          <div className={styles.emptyState}>
            <Star size={26} className={styles.emptyIcon} />
            <div className={styles.emptyText}>No favorite accounts yet</div>
            <Link className={styles.emptyLink} to={routes.rootPath}>
              Explore accounts
            </Link>
          </div>
        ) : (
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
                {favorites.map(favorite => (
                  <tr key={favorite.address} className={styles.tableRow}>
                    <td className={styles.accountCell}>
                      <InlineActionGroup>
                        <AddressChip
                          address={favorite.address}
                          fallback="Account"
                          copyable={false}
                          onAddressClick={(address, event) =>
                            openPath(routes.addressPath(address), event)
                          }
                        />
                        <InlineActionButton
                          type="button"
                          variant="danger"
                          onClick={() => handleRemove(favorite)}
                          aria-label="Remove from favorites"
                          title="Remove from favorites"
                        >
                          <Trash2 size={13} />
                        </InlineActionButton>
                      </InlineActionGroup>
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
                    <td className={styles.savedAtCell}>{formatSavedAt(favorite.savedAt)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </section>
  )
}

function formatSavedAt(savedAt: number): string {
  if (!savedAt) {
    return "Unknown"
  }
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(savedAt))
}

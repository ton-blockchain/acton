import {Address} from "@ton/core"
import {useCallback, useMemo, useSyncExternalStore} from "react"

import {useNetworkInfo} from "./useNetworkInfo"
import {createFavoritesStore} from "./favoritesStore"

export interface FavoriteAccount {
  readonly address: string
  readonly savedAt: number
}

const FAVORITE_ACCOUNTS_STORAGE_PREFIX = "acton:favorite-accounts"
const FAVORITE_ACCOUNTS_STORAGE_VERSION = "v1"
const FAVORITE_ACCOUNTS_CHANGE_EVENT = "acton:favorite-accounts-change"
const favoriteAccountsStore = createFavoritesStore<FavoriteAccount>({
  storagePrefix: FAVORITE_ACCOUNTS_STORAGE_PREFIX,
  storageVersion: FAVORITE_ACCOUNTS_STORAGE_VERSION,
  changeEvent: FAVORITE_ACCOUNTS_CHANGE_EVENT,
  parseRecord: parseFavoriteAccount,
  normalize: normalizeFavoriteAccounts,
})

export function useFavoriteAccounts() {
  const {network} = useNetworkInfo()
  const namespace = network.id
  const favorites = useSyncExternalStore(
    useCallback(
      onStoreChange => favoriteAccountsStore.subscribe(namespace, onStoreChange),
      [namespace],
    ),
    useCallback(() => favoriteAccountsStore.read(namespace), [namespace]),
    () => favoriteAccountsStore.empty,
  )
  const favoriteKeys = useMemo(
    () => new Set(favorites.map(favorite => favorite.address)),
    [favorites],
  )

  const isFavorite = useCallback(
    (address: string) => {
      const key = favoriteAddressKey(address)
      return key ? favoriteKeys.has(key) : false
    },
    [favoriteKeys],
  )

  const setFavorite = useCallback(
    (address: string, favorite: boolean) => setFavoriteAccount(namespace, address, favorite),
    [namespace],
  )

  const toggleFavorite = useCallback(
    (address: string) => toggleFavoriteAccount(namespace, address),
    [namespace],
  )

  return {
    favorites,
    isFavorite,
    setFavorite,
    toggleFavorite,
  }
}

function setFavoriteAccount(namespace: string, address: string, favorite: boolean): boolean {
  const key = favoriteAddressKey(address)
  if (!key) {
    return false
  }

  const current = favoriteAccountsStore.read(namespace)
  const currentWithoutAddress = current.filter(account => account.address !== key)
  const next = favorite
    ? [{address: key, savedAt: Date.now()}, ...currentWithoutAddress]
    : currentWithoutAddress

  favoriteAccountsStore.write(namespace, next)
  return favorite
}

function toggleFavoriteAccount(namespace: string, address: string): boolean {
  const key = favoriteAddressKey(address)
  if (!key) {
    return false
  }

  const current = favoriteAccountsStore.read(namespace)
  const isFavorite = current.some(account => account.address === key)
  return setFavoriteAccount(namespace, key, !isFavorite)
}

export function parseFavoriteAccounts(raw: string | null): readonly FavoriteAccount[] {
  return favoriteAccountsStore.parse(raw)
}

function normalizeFavoriteAccounts(
  favorites: readonly FavoriteAccount[],
): readonly FavoriteAccount[] {
  const seen = new Set<string>()
  const normalized: FavoriteAccount[] = []
  for (const favorite of favorites) {
    const address = favoriteAddressKey(favorite.address)
    if (!address || seen.has(address)) {
      continue
    }
    seen.add(address)
    normalized.push({
      address,
      savedAt: Number.isFinite(favorite.savedAt) && favorite.savedAt > 0 ? favorite.savedAt : 0,
    })
  }
  return normalized.sort((left, right) => right.savedAt - left.savedAt)
}

function favoriteAddressKey(address: string): string | undefined {
  const trimmed = address.trim()
  if (!trimmed) {
    return undefined
  }
  try {
    return Address.parse(trimmed).toRawString()
  } catch {
    return trimmed
  }
}

function parseFavoriteAccount(value: unknown): FavoriteAccount | undefined {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    typeof (value as FavoriteAccount).address !== "string" ||
    typeof (value as FavoriteAccount).savedAt !== "number"
  ) {
    return undefined
  }

  const address = favoriteAddressKey((value as FavoriteAccount).address)
  if (!address) {
    return undefined
  }

  const savedAt = (value as FavoriteAccount).savedAt
  return {
    address,
    savedAt: Number.isFinite(savedAt) && savedAt > 0 ? savedAt : 0,
  }
}

import {Address} from "@ton/core"
import {useCallback, useMemo, useSyncExternalStore} from "react"

import {useNetworkInfo} from "./useNetworkInfo"

export interface FavoriteAccount {
  readonly address: string
  readonly savedAt: number
}

interface FavoriteAccountsCacheEntry {
  readonly raw: string | null
  readonly value: readonly FavoriteAccount[]
}

const FAVORITE_ACCOUNTS_STORAGE_PREFIX = "acton:favorite-accounts"
const FAVORITE_ACCOUNTS_STORAGE_VERSION = "v1"
const FAVORITE_ACCOUNTS_CHANGE_EVENT = "acton:favorite-accounts-change"
const favoriteAccountsCache = new Map<string, FavoriteAccountsCacheEntry>()
const emptyFavorites: readonly FavoriteAccount[] = []

export function useFavoriteAccounts() {
  const {network} = useNetworkInfo()
  const namespace = network.id
  const favorites = useSyncExternalStore(
    useCallback(onStoreChange => subscribeFavoriteAccounts(namespace, onStoreChange), [namespace]),
    useCallback(() => readFavoriteAccounts(namespace), [namespace]),
    () => emptyFavorites,
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

function subscribeFavoriteAccounts(namespace: string, onStoreChange: () => void): () => void {
  const handleLocalChange = (event: Event) => {
    const detail = (event as CustomEvent<{readonly namespace?: string}>).detail
    if (!detail?.namespace || detail.namespace === namespace) {
      onStoreChange()
    }
  }
  const handleStorageChange = (event: StorageEvent) => {
    if (event.key === favoriteAccountsStorageKey(namespace)) {
      favoriteAccountsCache.delete(namespace)
      onStoreChange()
    }
  }

  globalThis.addEventListener?.(FAVORITE_ACCOUNTS_CHANGE_EVENT, handleLocalChange)
  globalThis.addEventListener?.("storage", handleStorageChange)

  return () => {
    globalThis.removeEventListener?.(FAVORITE_ACCOUNTS_CHANGE_EVENT, handleLocalChange)
    globalThis.removeEventListener?.("storage", handleStorageChange)
  }
}

function readFavoriteAccounts(namespace: string): readonly FavoriteAccount[] {
  const raw = readFavoriteAccountsRaw(namespace)
  const cached = favoriteAccountsCache.get(namespace)
  if (cached?.raw === raw) {
    return cached.value
  }

  const value = parseFavoriteAccounts(raw)
  favoriteAccountsCache.set(namespace, {raw, value})
  return value
}

function setFavoriteAccount(namespace: string, address: string, favorite: boolean): boolean {
  const key = favoriteAddressKey(address)
  if (!key) {
    return false
  }

  const current = readFavoriteAccounts(namespace)
  const currentWithoutAddress = current.filter(account => account.address !== key)
  const next = favorite
    ? [{address: key, savedAt: Date.now()}, ...currentWithoutAddress]
    : currentWithoutAddress

  writeFavoriteAccounts(namespace, next)
  return favorite
}

function toggleFavoriteAccount(namespace: string, address: string): boolean {
  const key = favoriteAddressKey(address)
  if (!key) {
    return false
  }

  const current = readFavoriteAccounts(namespace)
  const isFavorite = current.some(account => account.address === key)
  return setFavoriteAccount(namespace, key, !isFavorite)
}

function writeFavoriteAccounts(namespace: string, favorites: readonly FavoriteAccount[]): void {
  const key = favoriteAccountsStorageKey(namespace)
  const value = normalizeFavoriteAccounts(favorites)

  try {
    if (value.length > 0) {
      const raw = JSON.stringify(value)
      globalThis.localStorage?.setItem(key, raw)
      favoriteAccountsCache.set(namespace, {raw, value})
    } else {
      globalThis.localStorage?.removeItem(key)
      favoriteAccountsCache.set(namespace, {raw: null, value})
    }
  } catch {
    favoriteAccountsCache.set(namespace, {raw: readFavoriteAccountsRaw(namespace), value})
  }

  globalThis.dispatchEvent?.(new CustomEvent(FAVORITE_ACCOUNTS_CHANGE_EVENT, {detail: {namespace}}))
}

function readFavoriteAccountsRaw(namespace: string): string | null {
  try {
    return globalThis.localStorage?.getItem(favoriteAccountsStorageKey(namespace)) ?? null
  } catch {
    return null
  }
}

function parseFavoriteAccounts(raw: string | null): readonly FavoriteAccount[] {
  if (!raw) {
    return emptyFavorites
  }

  try {
    const parsed = JSON.parse(raw) as unknown
    if (!Array.isArray(parsed)) {
      return emptyFavorites
    }
    return normalizeFavoriteAccounts(
      parsed
        .map(entry => {
          if (!isFavoriteAccountRecord(entry)) {
            return undefined
          }
          const address = favoriteAddressKey(entry.address)
          if (!address) {
            return undefined
          }
          return {
            address,
            savedAt: Number.isFinite(entry.savedAt) && entry.savedAt > 0 ? entry.savedAt : 0,
          }
        })
        .filter((entry): entry is FavoriteAccount => Boolean(entry)),
    )
  } catch {
    return emptyFavorites
  }
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

function favoriteAccountsStorageKey(namespace: string): string {
  return `${FAVORITE_ACCOUNTS_STORAGE_PREFIX}:${namespace}:${FAVORITE_ACCOUNTS_STORAGE_VERSION}`
}

function isFavoriteAccountRecord(value: unknown): value is FavoriteAccount {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    typeof (value as FavoriteAccount).address === "string" &&
    typeof (value as FavoriteAccount).savedAt === "number"
  )
}

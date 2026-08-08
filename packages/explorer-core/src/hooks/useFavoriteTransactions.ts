import {Address} from "@ton/core"
import {useCallback, useMemo, useSyncExternalStore} from "react"

import {hashToHex} from "../components/utils"
import {createFavoritesStore} from "./favoritesStore"
import {useNetworkInfo} from "./useNetworkInfo"

export interface FavoriteTransactionInput {
  readonly hash: string
  readonly account?: string
  readonly lt?: string
  readonly timestamp?: number
}

export interface FavoriteTransaction extends FavoriteTransactionInput {
  readonly savedAt: number
}

const favoriteTransactionsStore = createFavoritesStore<FavoriteTransaction>({
  storagePrefix: "acton:favorite-transactions",
  storageVersion: "v1",
  changeEvent: "acton:favorite-transactions-change",
  parseRecord: parseFavoriteTransaction,
  normalize: normalizeFavoriteTransactions,
})

export function useFavoriteTransactions() {
  const {network} = useNetworkInfo()
  const namespace = network.id
  const favorites = useSyncExternalStore(
    useCallback(
      onStoreChange => favoriteTransactionsStore.subscribe(namespace, onStoreChange),
      [namespace],
    ),
    useCallback(() => favoriteTransactionsStore.read(namespace), [namespace]),
    () => favoriteTransactionsStore.empty,
  )
  const favoriteKeys = useMemo(() => new Set(favorites.map(favorite => favorite.hash)), [favorites])

  const isFavorite = useCallback(
    (hash: string) => {
      const key = favoriteTransactionHash(hash)
      return key ? favoriteKeys.has(key) : false
    },
    [favoriteKeys],
  )

  const setFavorite = useCallback(
    (transaction: FavoriteTransactionInput, favorite: boolean) =>
      setFavoriteTransaction(namespace, transaction, favorite),
    [namespace],
  )

  const toggleFavorite = useCallback(
    (transaction: FavoriteTransactionInput) => toggleFavoriteTransaction(namespace, transaction),
    [namespace],
  )

  return {favorites, isFavorite, setFavorite, toggleFavorite}
}

export function parseFavoriteTransactions(raw: string | null): readonly FavoriteTransaction[] {
  return favoriteTransactionsStore.parse(raw)
}

function setFavoriteTransaction(
  namespace: string,
  transaction: FavoriteTransactionInput,
  favorite: boolean,
): boolean {
  const normalized = normalizeFavoriteTransactionInput(transaction)
  if (!normalized) {
    return false
  }

  const current = favoriteTransactionsStore.read(namespace)
  const currentWithoutTransaction = current.filter(item => item.hash !== normalized.hash)
  const next = favorite
    ? [{...normalized, savedAt: Date.now()}, ...currentWithoutTransaction]
    : currentWithoutTransaction

  favoriteTransactionsStore.write(namespace, next)
  return favorite
}

function toggleFavoriteTransaction(
  namespace: string,
  transaction: FavoriteTransactionInput,
): boolean {
  const hash = favoriteTransactionHash(transaction.hash)
  if (!hash) {
    return false
  }

  const current = favoriteTransactionsStore.read(namespace)
  return setFavoriteTransaction(
    namespace,
    transaction,
    !current.some(favorite => favorite.hash === hash),
  )
}

function normalizeFavoriteTransactions(
  favorites: readonly FavoriteTransaction[],
): readonly FavoriteTransaction[] {
  const seen = new Set<string>()
  const normalized: FavoriteTransaction[] = []

  for (const favorite of favorites) {
    const transaction = normalizeFavoriteTransactionInput(favorite)
    if (!transaction || seen.has(transaction.hash)) {
      continue
    }

    seen.add(transaction.hash)
    normalized.push({
      ...transaction,
      savedAt: Number.isFinite(favorite.savedAt) && favorite.savedAt > 0 ? favorite.savedAt : 0,
    })
  }

  return normalized.sort((left, right) => right.savedAt - left.savedAt)
}

function normalizeFavoriteTransactionInput(
  transaction: FavoriteTransactionInput,
): FavoriteTransactionInput | undefined {
  const hash = favoriteTransactionHash(transaction.hash)
  if (!hash) {
    return undefined
  }

  const account = normalizeFavoriteTransactionAccount(transaction.account)
  const lt = transaction.lt?.trim() || undefined
  const timestamp =
    transaction.timestamp !== undefined &&
    Number.isFinite(transaction.timestamp) &&
    transaction.timestamp > 0
      ? transaction.timestamp
      : undefined

  return {hash, account, lt, timestamp}
}

function favoriteTransactionHash(hash: string): string | undefined {
  return hashToHex(hash)
}

function normalizeFavoriteTransactionAccount(account: string | undefined): string | undefined {
  const trimmed = account?.trim()
  if (!trimmed) {
    return undefined
  }

  try {
    return Address.parse(trimmed).toRawString()
  } catch {
    return trimmed
  }
}

function parseFavoriteTransaction(value: unknown): FavoriteTransaction | undefined {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    typeof (value as FavoriteTransaction).hash !== "string" ||
    typeof (value as FavoriteTransaction).savedAt !== "number"
  ) {
    return undefined
  }

  const candidate = value as FavoriteTransaction
  if (
    (candidate.account !== undefined && typeof candidate.account !== "string") ||
    (candidate.lt !== undefined && typeof candidate.lt !== "string") ||
    (candidate.timestamp !== undefined && typeof candidate.timestamp !== "number")
  ) {
    return undefined
  }

  const normalized = normalizeFavoriteTransactionInput(candidate)
  return normalized ? {...normalized, savedAt: candidate.savedAt} : undefined
}

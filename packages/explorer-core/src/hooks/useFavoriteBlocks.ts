import {useCallback, useMemo, useSyncExternalStore} from "react"

import {createFavoritesStore} from "./favoritesStore"
import {useNetworkInfo} from "./useNetworkInfo"

export interface FavoriteBlockInput {
  readonly workchain: number
  readonly shard: string
  readonly seqno: number
  readonly generatedAt?: number
}

export interface FavoriteBlock extends FavoriteBlockInput {
  readonly savedAt: number
}

const favoriteBlocksStore = createFavoritesStore<FavoriteBlock>({
  storagePrefix: "acton:favorite-blocks",
  storageVersion: "v1",
  changeEvent: "acton:favorite-blocks-change",
  parseRecord: parseFavoriteBlock,
  normalize: normalizeFavoriteBlocks,
})

export function useFavoriteBlocks() {
  const {network} = useNetworkInfo()
  const namespace = network.id
  const favorites = useSyncExternalStore(
    useCallback(
      onStoreChange => favoriteBlocksStore.subscribe(namespace, onStoreChange),
      [namespace],
    ),
    useCallback(() => favoriteBlocksStore.read(namespace), [namespace]),
    () => favoriteBlocksStore.empty,
  )
  const favoriteKeys = useMemo(
    () => new Set(favorites.map(favorite => favoriteBlockKey(favorite))),
    [favorites],
  )

  const isFavorite = useCallback(
    (block: FavoriteBlockInput) => {
      const key = favoriteBlockKey(block)
      return key ? favoriteKeys.has(key) : false
    },
    [favoriteKeys],
  )

  const setFavorite = useCallback(
    (block: FavoriteBlockInput, favorite: boolean) => setFavoriteBlock(namespace, block, favorite),
    [namespace],
  )

  const toggleFavorite = useCallback(
    (block: FavoriteBlockInput) => toggleFavoriteBlock(namespace, block),
    [namespace],
  )

  return {favorites, isFavorite, setFavorite, toggleFavorite}
}

export function parseFavoriteBlocks(raw: string | null): readonly FavoriteBlock[] {
  return favoriteBlocksStore.parse(raw)
}

function setFavoriteBlock(
  namespace: string,
  block: FavoriteBlockInput,
  favorite: boolean,
): boolean {
  const normalized = normalizeFavoriteBlockInput(block)
  if (!normalized) {
    return false
  }

  const key = favoriteBlockKey(normalized)
  const current = favoriteBlocksStore.read(namespace)
  const currentWithoutBlock = current.filter(item => favoriteBlockKey(item) !== key)
  const next = favorite
    ? [{...normalized, savedAt: Date.now()}, ...currentWithoutBlock]
    : currentWithoutBlock

  favoriteBlocksStore.write(namespace, next)
  return favorite
}

function toggleFavoriteBlock(namespace: string, block: FavoriteBlockInput): boolean {
  const key = favoriteBlockKey(block)
  if (!key) {
    return false
  }

  const current = favoriteBlocksStore.read(namespace)
  return setFavoriteBlock(
    namespace,
    block,
    !current.some(favorite => favoriteBlockKey(favorite) === key),
  )
}

function normalizeFavoriteBlocks(favorites: readonly FavoriteBlock[]): readonly FavoriteBlock[] {
  const seen = new Set<string>()
  const normalized: FavoriteBlock[] = []

  for (const favorite of favorites) {
    const block = normalizeFavoriteBlockInput(favorite)
    const key = block && favoriteBlockKey(block)
    if (!block || !key || seen.has(key)) {
      continue
    }

    seen.add(key)
    normalized.push({
      ...block,
      savedAt: Number.isFinite(favorite.savedAt) && favorite.savedAt > 0 ? favorite.savedAt : 0,
    })
  }

  return normalized.sort((left, right) => right.savedAt - left.savedAt)
}

function normalizeFavoriteBlockInput(block: FavoriteBlockInput): FavoriteBlockInput | undefined {
  const identity = normalizeFavoriteBlockIdentity(block)
  if (!identity) {
    return undefined
  }

  const generatedAt =
    block.generatedAt !== undefined && Number.isFinite(block.generatedAt) && block.generatedAt > 0
      ? block.generatedAt
      : undefined

  return {...identity, generatedAt}
}

function favoriteBlockKey(block: FavoriteBlockInput): string | undefined {
  const normalized = normalizeFavoriteBlockIdentity(block)
  return normalized ? `${normalized.workchain}:${normalized.shard}:${normalized.seqno}` : undefined
}

function normalizeFavoriteBlockIdentity(
  block: FavoriteBlockInput,
): Pick<FavoriteBlockInput, "workchain" | "shard" | "seqno"> | undefined {
  const shard = block.shard.trim().toLowerCase()
  if (
    !Number.isInteger(block.workchain) ||
    !shard ||
    !Number.isInteger(block.seqno) ||
    block.seqno < 0
  ) {
    return undefined
  }

  return {
    workchain: block.workchain,
    shard,
    seqno: block.seqno,
  }
}

function parseFavoriteBlock(value: unknown): FavoriteBlock | undefined {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    typeof (value as FavoriteBlock).workchain !== "number" ||
    typeof (value as FavoriteBlock).shard !== "string" ||
    typeof (value as FavoriteBlock).seqno !== "number" ||
    typeof (value as FavoriteBlock).savedAt !== "number"
  ) {
    return undefined
  }

  const candidate = value as FavoriteBlock
  if (candidate.generatedAt !== undefined && typeof candidate.generatedAt !== "number") {
    return undefined
  }

  const block = normalizeFavoriteBlockInput(candidate)
  return block ? {...block, savedAt: candidate.savedAt} : undefined
}

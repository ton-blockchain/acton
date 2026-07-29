interface FavoriteRecord {
  readonly savedAt: number
}

interface FavoritesStoreOptions<T extends FavoriteRecord> {
  readonly storagePrefix: string
  readonly storageVersion: string
  readonly changeEvent: string
  readonly parseRecord: (value: unknown) => T | undefined
  readonly normalize: (favorites: readonly T[]) => readonly T[]
}

interface FavoritesCacheEntry<T> {
  readonly raw: string | null
  readonly value: readonly T[]
}

export interface FavoritesStore<T extends FavoriteRecord> {
  readonly empty: readonly T[]
  readonly parse: (raw: string | null) => readonly T[]
  readonly read: (namespace: string) => readonly T[]
  readonly subscribe: (namespace: string, onStoreChange: () => void) => () => void
  readonly write: (namespace: string, favorites: readonly T[]) => void
}

export function createFavoritesStore<T extends FavoriteRecord>(
  options: FavoritesStoreOptions<T>,
): FavoritesStore<T> {
  const cache = new Map<string, FavoritesCacheEntry<T>>()
  const empty: readonly T[] = []

  const storageKey = (namespace: string): string =>
    `${options.storagePrefix}:${namespace}:${options.storageVersion}`

  const readRaw = (namespace: string): string | null => {
    try {
      return globalThis.localStorage?.getItem(storageKey(namespace)) ?? null
    } catch {
      return null
    }
  }

  const parse = (raw: string | null): readonly T[] => {
    if (!raw) {
      return empty
    }

    try {
      const parsed = JSON.parse(raw) as unknown
      if (!Array.isArray(parsed)) {
        return empty
      }

      return options.normalize(
        parsed.map(options.parseRecord).filter((entry): entry is T => entry !== undefined),
      )
    } catch {
      return empty
    }
  }

  const read = (namespace: string): readonly T[] => {
    const raw = readRaw(namespace)
    const cached = cache.get(namespace)
    if (cached?.raw === raw) {
      return cached.value
    }

    const value = parse(raw)
    cache.set(namespace, {raw, value})
    return value
  }

  const write = (namespace: string, favorites: readonly T[]): void => {
    const key = storageKey(namespace)
    const value = options.normalize(favorites)

    try {
      if (value.length > 0) {
        const raw = JSON.stringify(value)
        globalThis.localStorage?.setItem(key, raw)
        cache.set(namespace, {raw, value})
      } else {
        globalThis.localStorage?.removeItem(key)
        cache.set(namespace, {raw: null, value})
      }
    } catch {
      cache.set(namespace, {raw: readRaw(namespace), value})
    }

    globalThis.dispatchEvent?.(new CustomEvent(options.changeEvent, {detail: {namespace}}))
  }

  const subscribe = (namespace: string, onStoreChange: () => void): (() => void) => {
    const handleLocalChange = (event: Event) => {
      const detail = (event as CustomEvent<{readonly namespace?: string}>).detail
      if (!detail?.namespace || detail.namespace === namespace) {
        onStoreChange()
      }
    }
    const handleStorageChange = (event: StorageEvent) => {
      if (event.key === storageKey(namespace)) {
        cache.delete(namespace)
        onStoreChange()
      }
    }

    globalThis.addEventListener?.(options.changeEvent, handleLocalChange)
    globalThis.addEventListener?.("storage", handleStorageChange)

    return () => {
      globalThis.removeEventListener?.(options.changeEvent, handleLocalChange)
      globalThis.removeEventListener?.("storage", handleStorageChange)
    }
  }

  return {empty, parse, read, subscribe, write}
}

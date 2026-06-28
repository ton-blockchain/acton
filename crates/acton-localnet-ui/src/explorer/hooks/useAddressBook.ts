import {Address} from "@ton/core"
import {
  createContext,
  createElement,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react"
import type {FC, ReactNode} from "react"

import {useMetadataRegistry} from "../metadata/MetadataRegistryProvider"

type AddressName = string | undefined

interface TonAssetsAccount {
  readonly address: string
  readonly name: string
}

export interface TonAssetsNameMatch {
  readonly address: string
  readonly name: string
}

interface AddressBookContextValue {
  readonly getCachedName: (address: string) => AddressName | undefined
  readonly fetchName: (address: string) => Promise<AddressName>
  readonly prefetchNames: (addresses: readonly string[]) => Promise<void>
  readonly searchTonAssetsNames: (query: string, limit?: number) => readonly TonAssetsNameMatch[]
  readonly updateName: (address: string, name: AddressName) => void
  readonly setAddressName: (address: string, name: string) => Promise<void>
  readonly version: number
}

const AddressBookContext = createContext<AddressBookContextValue | undefined>(undefined)
const TON_ASSETS_ACCOUNTS_URL =
  "https://raw.githubusercontent.com/tonkeeper/ton-assets/main/accounts.json"
const TON_ASSETS_ACCOUNTS_CACHE_KEY = "acton.tonAssets.accounts.v1"
const TON_ASSETS_ACCOUNTS_CACHE_TTL_MS = 24 * 60 * 60 * 1000

interface TonAssetsAccountsCache {
  readonly savedAt: number
  readonly accounts: readonly TonAssetsAccount[]
}

const normalizeKey = (address: string) => {
  try {
    return Address.parse(address).toRawString()
  } catch {
    return address
  }
}

interface PendingNameRequest {
  readonly address: string
  readonly resolve: (name: AddressName) => void
}

export const AddressBookProvider: FC<{
  children: ReactNode
}> = ({children}) => {
  const metadataRegistry = useMetadataRegistry()
  const cacheRef = useRef(new Map<string, AddressName>())
  const tonAssetsRef = useRef(new Map<string, string>())
  const tonAssetsAccountsRef = useRef<readonly TonAssetsNameMatch[]>([])
  const pendingRef = useRef(new Map<string, Promise<AddressName>>())
  const pendingBatchRef = useRef(new Map<string, PendingNameRequest>())
  const batchScheduledRef = useRef(false)
  const [version, setVersion] = useState(0)

  const getCachedName = useCallback((address: string) => {
    if (!address) return
    const key = normalizeKey(address)
    if (cacheRef.current.has(key)) {
      return cacheRef.current.get(key) ?? tonAssetsRef.current.get(key)
    }
    return tonAssetsRef.current.get(key)
  }, [])

  const updateNames = useCallback((entries: readonly (readonly [string, AddressName])[]) => {
    if (entries.length === 0) return
    for (const [address, name] of entries) {
      if (!address) continue
      cacheRef.current.set(normalizeKey(address), name)
    }
    setVersion(prev => prev + 1)
  }, [])

  const updateName = useCallback(
    (address: string, name: AddressName) => updateNames([[address, name]]),
    [updateNames],
  )

  useEffect(() => {
    let isActive = true

    const loadTonAssetsAccounts = async () => {
      try {
        const cached = readTonAssetsAccountsCache()
        if (cached) {
          const nextAccounts = buildTonAssetsAccounts(cached.accounts)
          const next = buildTonAssetsAccountsMap(nextAccounts)
          if (next.size > 0) {
            tonAssetsRef.current = next
            tonAssetsAccountsRef.current = nextAccounts
            setVersion(prev => prev + 1)
          }
          if (Date.now() - cached.savedAt < TON_ASSETS_ACCOUNTS_CACHE_TTL_MS) {
            return
          }
        }

        const response = await fetch(TON_ASSETS_ACCOUNTS_URL)
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`)
        }

        const accounts = (await response.json()) as unknown
        if (!Array.isArray(accounts)) {
          throw new TypeError("ton-assets accounts.json must be an array.")
        }

        const validAccounts = accounts.filter(isTonAssetsAccount)
        const nextAccounts = buildTonAssetsAccounts(validAccounts)
        const next = buildTonAssetsAccountsMap(nextAccounts)

        if (isActive && next.size > 0) {
          tonAssetsRef.current = next
          tonAssetsAccountsRef.current = nextAccounts
          writeTonAssetsAccountsCache(validAccounts)
          setVersion(prev => prev + 1)
        }
      } catch (error) {
        console.warn("Failed to load ton-assets account names:", error)
      }
    }

    void loadTonAssetsAccounts()
    return () => {
      isActive = false
    }
  }, [])

  const flushPendingBatch = useCallback(() => {
    batchScheduledRef.current = false
    const requests = [...pendingBatchRef.current.values()]
    pendingBatchRef.current.clear()

    if (requests.length === 0) {
      return
    }

    void metadataRegistry
      .getAddressNames(requests.map(request => request.address))
      .then(namesByAddress => {
        const entries = requests.map(request => {
          return [request.address, namesByAddress[request.address]] as const
        })
        updateNames(entries)
        for (const request of requests) {
          request.resolve(
            namesByAddress[request.address] ??
              tonAssetsRef.current.get(normalizeKey(request.address)),
          )
        }
      })
      .catch(error => {
        console.warn("Failed to fetch address names:", error)
        const missingName: AddressName = undefined
        const entries = requests.map(request => [request.address, undefined] as const)
        updateNames(entries)
        for (const request of requests) {
          request.resolve(missingName ?? tonAssetsRef.current.get(normalizeKey(request.address)))
        }
      })
  }, [metadataRegistry, updateNames])

  const setAddressName = useCallback(
    async (address: string, name: string) => {
      await metadataRegistry.setAddressName(address, name || undefined)
      updateName(address, name || undefined)
    },
    [metadataRegistry, updateName],
  )

  const fetchName = useCallback(
    async (address: string) => {
      if (!address) return
      const key = normalizeKey(address)
      if (cacheRef.current.has(key)) {
        return cacheRef.current.get(key) ?? tonAssetsRef.current.get(key)
      }
      const pending = pendingRef.current.get(key)
      if (pending) return pending

      const request = new Promise<AddressName>(resolve => {
        pendingBatchRef.current.set(key, {address, resolve})
        if (!batchScheduledRef.current) {
          batchScheduledRef.current = true
          globalThis.queueMicrotask(flushPendingBatch)
        }
      }).finally(() => {
        pendingRef.current.delete(key)
      })

      pendingRef.current.set(key, request)
      return request
    },
    [flushPendingBatch],
  )

  const prefetchNames = useCallback(
    async (addresses: readonly string[]) => {
      await Promise.all(addresses.map(address => fetchName(address)))
    },
    [fetchName],
  )

  const searchTonAssetsNames = useCallback((query: string, limit = 6) => {
    const normalizedQuery = normalizeNameQuery(query)
    if (normalizedQuery.length < 2 || limit <= 0) {
      return []
    }

    return tonAssetsAccountsRef.current
      .map(account => {
        const normalizedName = normalizeNameQuery(account.name)
        if (!normalizedName.includes(normalizedQuery)) {
          return undefined
        }

        return {
          account,
          score: getNameMatchScore(normalizedName, normalizedQuery),
        }
      })
      .filter((entry): entry is {readonly account: TonAssetsNameMatch; readonly score: number} =>
        Boolean(entry),
      )
      .sort((a, b) => a.score - b.score || a.account.name.localeCompare(b.account.name))
      .slice(0, limit)
      .map(entry => entry.account)
  }, [])

  const value = useMemo(
    () => ({
      getCachedName,
      fetchName,
      prefetchNames,
      searchTonAssetsNames,
      updateName,
      setAddressName,
      version,
    }),
    [
      fetchName,
      getCachedName,
      prefetchNames,
      searchTonAssetsNames,
      setAddressName,
      updateName,
      version,
    ],
  )

  return createElement(AddressBookContext.Provider, {value}, children)
}

export const useAddressBook = () => {
  const ctx = useContext(AddressBookContext)
  if (!ctx) {
    throw new Error("useAddressBook must be used within AddressBookProvider")
  }
  return ctx
}

export const useAddressName = (address: string) => {
  const {getCachedName, fetchName, version} = useAddressBook()
  const [name, setName] = useState<AddressName>(() => getCachedName(address))

  useEffect(() => {
    setName(getCachedName(address))
  }, [address, getCachedName, version])

  useEffect(() => {
    if (!address) {
      setName(undefined)
      return
    }
    let isActive = true
    const cached = getCachedName(address)
    void fetchName(address).then(next => {
      if (isActive) setName(next ?? cached)
    })
    return () => {
      isActive = false
    }
  }, [address, fetchName, getCachedName])

  return name
}

function buildTonAssetsAccounts(
  accounts: readonly TonAssetsAccount[],
): readonly TonAssetsNameMatch[] {
  return accounts.map(account => ({
    address: normalizeKey(account.address),
    name: account.name,
  }))
}

function buildTonAssetsAccountsMap(accounts: readonly TonAssetsNameMatch[]): Map<string, string> {
  const next = new Map<string, string>()
  for (const account of accounts) {
    next.set(account.address, account.name)
  }
  return next
}

function normalizeNameQuery(value: string): string {
  return value.trim().toLocaleLowerCase()
}

function getNameMatchScore(normalizedName: string, normalizedQuery: string): number {
  if (normalizedName === normalizedQuery) {
    return 0
  }
  if (normalizedName.startsWith(normalizedQuery)) {
    return 1
  }
  return 2
}

function readTonAssetsAccountsCache(): TonAssetsAccountsCache | undefined {
  try {
    const raw = globalThis.localStorage?.getItem(TON_ASSETS_ACCOUNTS_CACHE_KEY)
    if (!raw) return undefined
    return JSON.parse(raw) as TonAssetsAccountsCache
  } catch {
    return undefined
  }
}

function writeTonAssetsAccountsCache(accounts: readonly TonAssetsAccount[]): void {
  try {
    globalThis.localStorage?.setItem(
      TON_ASSETS_ACCOUNTS_CACHE_KEY,
      JSON.stringify({savedAt: Date.now(), accounts}),
    )
  } catch {
    // Ignore storage quota and privacy-mode errors; network loading still works for this session.
  }
}

function isTonAssetsAccount(value: unknown): value is TonAssetsAccount {
  return (
    value !== null &&
    typeof value === "object" &&
    "address" in value &&
    "name" in value &&
    typeof value.address === "string" &&
    typeof value.name === "string" &&
    value.address.length > 0 &&
    value.name.length > 0
  )
}

import {getMainnetAddresses, getMainnetJettons, getTestnetAddresses} from "@acton/address-registry"
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
import {useNetworkInfo} from "./useNetworkInfo"

type AddressName = string | undefined

export interface AddressNameSources {
  readonly customName?: string
  readonly registryName?: string
  readonly tonDnsName?: string
}

export function resolveAddressName(
  customName: AddressName,
  registryName: AddressName,
  domainName: AddressName,
): AddressName {
  return customName ?? registryName ?? domainName
}

interface AddressBookDomainRow {
  readonly domain?: string | null
}

export interface RegistryNameMatch {
  readonly address: string
  readonly image?: string
  readonly kind?: "token"
  readonly name: string
  readonly symbol?: string
}

interface AddressBookContextValue {
  readonly localAddressNames: readonly RegistryNameMatch[]
  readonly getNameSources: (address: string) => AddressNameSources
  readonly getCachedName: (address: string) => AddressName | undefined
  readonly fetchName: (address: string) => Promise<AddressName>
  readonly prefetchNames: (addresses: readonly string[]) => Promise<void>
  readonly searchRegistryNames: (query: string, limit?: number) => readonly RegistryNameMatch[]
  readonly updateName: (address: string, name: AddressName) => void
  readonly updateDomains: (addressBook: Readonly<Record<string, AddressBookDomainRow>>) => void
  readonly setAddressName: (address: string, name: string) => Promise<void>
  readonly version: number
}

const AddressBookContext = createContext<AddressBookContextValue | undefined>(undefined)

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
  const {network} = useNetworkInfo()
  const registryAddresses = network.testOnly ? getTestnetAddresses() : getMainnetAddresses()
  const registryJettons = useMemo(
    () => (network.testOnly ? [] : getMainnetJettons()),
    [network.testOnly],
  )
  const registrySearchEntries = useMemo<readonly RegistryNameMatch[]>(
    () => [
      ...registryAddresses,
      ...registryJettons.map(jetton => ({...jetton, kind: "token" as const})),
    ],
    [registryAddresses, registryJettons],
  )
  const registryNames = useMemo(
    () => new Map(registrySearchEntries.map(({address, name}) => [address, name])),
    [registrySearchEntries],
  )
  const cacheRef = useRef(new Map<string, AddressName>())
  const domainsRef = useRef(new Map<string, string>())
  const pendingRef = useRef(new Map<string, Promise<AddressName>>())
  const pendingBatchRef = useRef(new Map<string, PendingNameRequest>())
  const batchScheduledRef = useRef(false)
  const [version, setVersion] = useState(0)
  const [storedAddressNames, setStoredAddressNames] = useState<readonly RegistryNameMatch[]>([])

  const getNameSources = useCallback(
    (address: string): AddressNameSources => {
      if (!address) return {}
      const key = normalizeKey(address)
      return {
        customName: cacheRef.current.get(key),
        registryName: registryNames.get(key),
        tonDnsName: domainsRef.current.get(key),
      }
    },
    [registryNames],
  )

  const getCachedName = useCallback(
    (address: string) => {
      const sources = getNameSources(address)
      return resolveAddressName(sources.customName, sources.registryName, sources.tonDnsName)
    },
    [getNameSources],
  )

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
    if (!metadataRegistry.listAddressNames) {
      setStoredAddressNames([])
      return
    }

    void metadataRegistry
      .listAddressNames()
      .then(entries => {
        if (!isActive) return
        const matches = entries
          .filter(entry => entry.address && entry.name.trim())
          .map(entry => ({address: entry.address, name: entry.name.trim()}))
        setStoredAddressNames(matches)
        updateNames(matches.map(entry => [entry.address, entry.name] as const))
      })
      .catch(() => {
        if (isActive) {
          setStoredAddressNames([])
        }
      })

    return () => {
      isActive = false
    }
  }, [metadataRegistry, updateNames])

  const updateDomains = useCallback(
    (addressBook: Readonly<Record<string, AddressBookDomainRow>>) => {
      let changed = false
      for (const [address, row] of Object.entries(addressBook)) {
        if (!address) continue
        const key = normalizeKey(address)
        const domain = row.domain?.trim() || undefined
        if (domain) {
          if (domainsRef.current.get(key) !== domain) {
            domainsRef.current.set(key, domain)
            changed = true
          }
        } else if (domainsRef.current.delete(key)) {
          changed = true
        }
      }
      if (changed) {
        setVersion(prev => prev + 1)
      }
    },
    [],
  )

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
            resolveAddressName(
              namesByAddress[request.address],
              registryNames.get(normalizeKey(request.address)),
              domainsRef.current.get(normalizeKey(request.address)),
            ),
          )
        }
      })
      .catch(error => {
        console.warn("Failed to fetch address names:", error)
        const entries = requests.map(request => [request.address, undefined] as const)
        updateNames(entries)
        for (const request of requests) {
          const key = normalizeKey(request.address)
          request.resolve(
            resolveAddressName(undefined, registryNames.get(key), domainsRef.current.get(key)),
          )
        }
      })
  }, [metadataRegistry, registryNames, updateNames])

  const setAddressName = useCallback(
    async (address: string, name: string) => {
      await metadataRegistry.setAddressName(address, name || undefined)
      const nextName = name.trim() || undefined
      updateName(address, nextName)
      setStoredAddressNames(current => {
        const key = normalizeKey(address)
        const withoutAddress = current.filter(entry => normalizeKey(entry.address) !== key)
        return nextName ? [{address, name: nextName}, ...withoutAddress] : withoutAddress
      })
    },
    [metadataRegistry, updateName],
  )

  const fetchName = useCallback(
    async (address: string) => {
      if (!address) return
      const key = normalizeKey(address)
      if (cacheRef.current.has(key)) {
        return resolveAddressName(
          cacheRef.current.get(key),
          registryNames.get(key),
          domainsRef.current.get(key),
        )
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
    [flushPendingBatch, registryNames],
  )

  const prefetchNames = useCallback(
    async (addresses: readonly string[]) => {
      await Promise.all(addresses.map(address => fetchName(address)))
    },
    [fetchName],
  )

  const searchRegistryNames = useCallback(
    (query: string, limit = 6) => {
      const namesByAddress = new Map<string, RegistryNameMatch>()
      for (const account of registrySearchEntries) {
        namesByAddress.set(normalizeKey(account.address), account)
      }
      for (const entry of storedAddressNames) {
        namesByAddress.set(normalizeKey(entry.address), entry)
      }
      return searchAddressNames([...namesByAddress.values()], query, limit)
    },
    [registrySearchEntries, storedAddressNames],
  )

  const value = useMemo(
    () => ({
      localAddressNames: storedAddressNames,
      getNameSources,
      getCachedName,
      fetchName,
      prefetchNames,
      searchRegistryNames,
      updateName,
      updateDomains,
      setAddressName,
      version,
    }),
    [
      fetchName,
      getCachedName,
      getNameSources,
      storedAddressNames,
      prefetchNames,
      searchRegistryNames,
      setAddressName,
      updateDomains,
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

export const useAddressNameSources = (address: string): AddressNameSources => {
  const {getNameSources, version} = useAddressBook()
  return useMemo(() => getNameSources(address), [address, getNameSources, version])
}

function normalizeNameQuery(value: string): string {
  return value.trim().toLocaleLowerCase().replaceAll("₮", "t")
}

export function searchAddressNames(
  entries: readonly RegistryNameMatch[],
  query: string,
  limit: number,
): readonly RegistryNameMatch[] {
  const normalizedQuery = normalizeNameQuery(query)
  if (normalizedQuery.length < 2 || limit <= 0) {
    return []
  }

  return entries
    .map(entry => {
      const normalizedName = normalizeNameQuery(entry.name)
      const normalizedSymbol = entry.symbol ? normalizeNameQuery(entry.symbol) : undefined
      const searchableValues = normalizedSymbol
        ? [normalizedName, normalizedSymbol]
        : [normalizedName]
      if (!searchableValues.some(value => value.includes(normalizedQuery))) {
        return undefined
      }

      return {
        entry,
        score: Math.min(
          ...searchableValues.map(value => getNameMatchScore(value, normalizedQuery)),
        ),
      }
    })
    .filter((entry): entry is {readonly entry: RegistryNameMatch; readonly score: number} =>
      Boolean(entry),
    )
    .sort((a, b) => a.score - b.score || a.entry.name.localeCompare(b.entry.name))
    .slice(0, limit)
    .map(entry => entry.entry)
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

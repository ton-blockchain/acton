import {addresses as registryAddresses} from "@acton/address-registry"
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
  readonly name: string
}

interface AddressBookContextValue {
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
const REGISTRY_NAMES = new Map(registryAddresses.map(({address, name}) => [address, name]))

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
  const domainsRef = useRef(new Map<string, string>())
  const pendingRef = useRef(new Map<string, Promise<AddressName>>())
  const pendingBatchRef = useRef(new Map<string, PendingNameRequest>())
  const batchScheduledRef = useRef(false)
  const [version, setVersion] = useState(0)

  const getNameSources = useCallback((address: string): AddressNameSources => {
    if (!address) return {}
    const key = normalizeKey(address)
    return {
      customName: cacheRef.current.get(key),
      registryName: REGISTRY_NAMES.get(key),
      tonDnsName: domainsRef.current.get(key),
    }
  }, [])

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
              REGISTRY_NAMES.get(normalizeKey(request.address)),
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
            resolveAddressName(undefined, REGISTRY_NAMES.get(key), domainsRef.current.get(key)),
          )
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
        return resolveAddressName(
          cacheRef.current.get(key),
          REGISTRY_NAMES.get(key),
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
    [flushPendingBatch],
  )

  const prefetchNames = useCallback(
    async (addresses: readonly string[]) => {
      await Promise.all(addresses.map(address => fetchName(address)))
    },
    [fetchName],
  )

  const searchRegistryNames = useCallback((query: string, limit = 6) => {
    const normalizedQuery = normalizeNameQuery(query)
    if (normalizedQuery.length < 2 || limit <= 0) {
      return []
    }

    return registryAddresses
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
      .filter((entry): entry is {readonly account: RegistryNameMatch; readonly score: number} =>
        Boolean(entry),
      )
      .sort((a, b) => a.score - b.score || a.account.name.localeCompare(b.account.name))
      .slice(0, limit)
      .map(entry => entry.account)
  }, [])

  const value = useMemo(
    () => ({
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

import {Address} from "@ton/core"
import type React from "react"
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

import type {TonClient} from "../api/client"

type AddressName = string | undefined

interface AddressBookContextValue {
  readonly getCachedName: (address: string) => AddressName | undefined
  readonly fetchName: (address: string) => Promise<AddressName>
  readonly prefetchNames: (addresses: readonly string[]) => Promise<void>
  readonly updateName: (address: string, name: AddressName) => void
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

export const AddressBookProvider: React.FC<{
  client: TonClient
  children: React.ReactNode
}> = ({client, children}) => {
  const cacheRef = useRef(new Map<string, AddressName>())
  const pendingRef = useRef(new Map<string, Promise<AddressName>>())
  const pendingBatchRef = useRef(new Map<string, PendingNameRequest>())
  const batchScheduledRef = useRef(false)
  const [version, setVersion] = useState(0)

  const getCachedName = useCallback((address: string) => {
    if (!address) return
    const key = normalizeKey(address)
    return cacheRef.current.get(key)
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

  const flushPendingBatch = useCallback(() => {
    batchScheduledRef.current = false
    const requests = [...pendingBatchRef.current.values()]
    pendingBatchRef.current.clear()

    if (requests.length === 0) {
      return
    }

    void client
      .getAddressNames(requests.map(request => request.address))
      .then(namesByAddress => {
        const entries = requests.map(request => {
          return [request.address, namesByAddress[request.address]] as const
        })
        updateNames(entries)
        for (const request of requests) {
          request.resolve(namesByAddress[request.address])
        }
      })
      .catch(error => {
        console.warn("Failed to fetch address names:", error)
        const missingName: AddressName = undefined
        const entries = requests.map(request => [request.address, undefined] as const)
        updateNames(entries)
        for (const request of requests) {
          request.resolve(missingName)
        }
      })
  }, [client, updateNames])

  const setAddressName = useCallback(
    async (address: string, name: string) => {
      await client.setAddressName(address, name)
      updateName(address, name || undefined)
    },
    [client, updateName],
  )

  const fetchName = useCallback(
    async (address: string) => {
      if (!address) return
      const key = normalizeKey(address)
      if (cacheRef.current.has(key)) {
        return cacheRef.current.get(key)
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

  const value = useMemo(
    () => ({
      getCachedName,
      fetchName,
      prefetchNames,
      updateName,
      setAddressName,
      version,
    }),
    [fetchName, getCachedName, prefetchNames, setAddressName, updateName, version],
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
    if (cached === undefined) {
      void fetchName(address).then(next => {
        if (isActive) setName(next)
      })
    }
    return () => {
      isActive = false
    }
  }, [address, fetchName, getCachedName])

  return name
}

import {useEffect, useRef, useState} from "react"
import type {BackendContractInfo} from "@acton/transaction-ui"

import {useTestUiApi} from "../testUiApiContext"
import {isAbortError} from "./request"

export function useContracts(contractNames: string[]) {
  const {baseUrl, url} = useTestUiApi()
  const [contracts, setContracts] = useState<Record<string, BackendContractInfo>>({})
  const [loading, setLoading] = useState(false)
  const fetchedNames = useRef<Set<string>>(new Set())

  useEffect(() => {
    fetchedNames.current.clear()
    setContracts({})
  }, [baseUrl])

  useEffect(() => {
    const namesToFetch = contractNames.filter(
      name => !fetchedNames.current.has(`${baseUrl}:${name}`),
    )

    if (namesToFetch.length === 0) return

    const controller = new AbortController()
    setLoading(true)

    const loadContracts = async () => {
      try {
        const results = await Promise.all(
          namesToFetch.map(async name => {
            try {
              const response = await fetch(url(`/contract/${encodeURIComponent(name)}`), {
                signal: controller.signal,
              })
              if (!response.ok) throw new Error(`Failed to fetch contract ${name}`)

              return {name, data: (await response.json()) as BackendContractInfo}
            } catch (error) {
              if (isAbortError(error)) throw error

              console.error(error)
              return {name, data: undefined}
            }
          }),
        )

        setContracts(previous => {
          const next = {...previous}
          for (const {name, data} of results) {
            if (data) next[name] = data
            fetchedNames.current.add(`${baseUrl}:${name}`)
          }
          return next
        })
        setLoading(false)
      } catch (error) {
        if (!isAbortError(error)) {
          console.error("Failed to fetch contracts", error)
          setLoading(false)
        }
      }
    }

    void loadContracts()
    return () => controller.abort()
  }, [baseUrl, contractNames, url])

  return {contracts, loading}
}

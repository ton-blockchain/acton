import { useEffect, useRef, useState } from "react"
import type { BackendContractInfo } from "@acton/shared-ui"

export function useContracts(contractNames: string[]) {
  const [contracts, setContracts] = useState<Record<string, BackendContractInfo>>({})
  const [loading, setLoading] = useState(false)
  const fetchedNames = useRef<Set<string>>(new Set())

  useEffect(() => {
    const namesToFetch = contractNames.filter((name) => !fetchedNames.current.has(name))

    if (namesToFetch.length === 0) return

    setLoading(true)

    Promise.all(
      namesToFetch.map((name) =>
        fetch(`/api/contract/${name}`)
          .then((res) => {
            if (!res.ok) throw new Error(`Failed to fetch contract ${name}`)
            return res.json()
          })
          .then((data) => ({ name, data }))
          .catch((err) => {
            console.error(err)
            return { name, data: null }
          }),
      ),
    ).then((results) => {
      setContracts((prev) => {
        const next = { ...prev }
        for (const { name, data } of results) {
          if (data) {
            next[name] = data
          }
          fetchedNames.current.add(name)
        }
        return next
      })
      setLoading(false)
    })
  }, [contractNames])

  return { contracts, loading }
}

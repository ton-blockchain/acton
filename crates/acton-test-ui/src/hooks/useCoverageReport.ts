import {useEffect, useState} from "react"

import {getErrorMessage, isAbortError} from "./request"

interface CoverageReportState {
  readonly lcov: string | undefined
  readonly error: string | undefined
  readonly loading: boolean
}

export function useCoverageReport() {
  const [state, setState] = useState<CoverageReportState>({
    lcov: undefined,
    error: undefined,
    loading: true,
  })

  useEffect(() => {
    const controller = new AbortController()

    const loadCoverage = async () => {
      try {
        const response = await fetch("/api/coverage.lcov", {signal: controller.signal})
        if (response.status === 204) {
          setState({lcov: undefined, error: undefined, loading: false})
          return
        }
        if (!response.ok) {
          throw new Error(`Failed to fetch coverage report: ${response.status}`)
        }

        const lcov = await response.text()
        setState({lcov, error: undefined, loading: false})
      } catch (error) {
        if (isAbortError(error)) return

        console.error("Failed to fetch coverage report", error)
        setState({lcov: undefined, error: getErrorMessage(error), loading: false})
      }
    }

    void loadCoverage()
    return () => controller.abort()
  }, [])

  return state
}

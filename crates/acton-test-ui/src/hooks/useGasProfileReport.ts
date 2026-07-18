import {useEffect, useState} from "react"

import type {GasProfileReport} from "../components/GasProfile/GasProfile"
import {getErrorMessage, isAbortError} from "./request"

interface GasProfileReportState {
  readonly profile: GasProfileReport | undefined
  readonly error: string | undefined
  readonly loading: boolean
  readonly loaded: boolean
}

const DISABLED_STATE: GasProfileReportState = {
  profile: undefined,
  error: undefined,
  loading: false,
  loaded: false,
}

export function useGasProfileReport(enabled = true) {
  const [state, setState] = useState<GasProfileReportState>(DISABLED_STATE)

  useEffect(() => {
    if (!enabled) {
      setState(DISABLED_STATE)
      return
    }

    const controller = new AbortController()
    setState({profile: undefined, error: undefined, loading: true, loaded: false})

    const loadGasProfile = async () => {
      try {
        const response = await fetch("/api/gas-profile", {signal: controller.signal})
        if (response.status === 204) {
          setState({profile: undefined, error: undefined, loading: false, loaded: true})
          return
        }
        if (!response.ok) {
          throw new Error(`Failed to fetch gas profile: ${response.status}`)
        }

        const profile = (await response.json()) as GasProfileReport
        setState({profile, error: undefined, loading: false, loaded: true})
      } catch (error) {
        if (isAbortError(error)) return

        console.error("Failed to fetch gas profile", error)
        setState({profile: undefined, error: getErrorMessage(error), loading: false, loaded: true})
      }
    }

    void loadGasProfile()
    return () => controller.abort()
  }, [enabled])

  return enabled ? state : DISABLED_STATE
}

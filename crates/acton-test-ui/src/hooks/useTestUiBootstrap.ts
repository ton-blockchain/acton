import {useEffect, useState} from "react"

import type {TestReport} from "../types/test"

import {isAbortError} from "./request"

interface TestUiBootstrapState {
  readonly reports: TestReport[]
  readonly reportsLoading: boolean
  readonly projectRoot: string
  readonly capabilitiesLoaded: boolean
  readonly coverageAvailable: boolean
  readonly gasProfileAvailable: boolean
}

const INITIAL_STATE: TestUiBootstrapState = {
  reports: [],
  reportsLoading: true,
  projectRoot: "",
  capabilitiesLoaded: false,
  coverageAvailable: false,
  gasProfileAvailable: false,
}

export function useTestUiBootstrap(onConnected: () => void) {
  const [state, setState] = useState<TestUiBootstrapState>(INITIAL_STATE)

  useEffect(() => {
    const controller = new AbortController()

    const loadConfig = async () => {
      try {
        const response = await fetch("/api/config", {signal: controller.signal})
        if (!response.ok) throw new Error(`Failed to fetch config: ${response.status}`)

        const config = (await response.json()) as {
          project_root: string
          coverage_available?: boolean
          gas_profile_available?: boolean
        }
        onConnected()
        setState(previous => ({
          ...previous,
          projectRoot: config.project_root,
          capabilitiesLoaded: true,
          coverageAvailable: config.coverage_available === true,
          gasProfileAvailable: config.gas_profile_available === true,
        }))
      } catch (error) {
        if (isAbortError(error)) return

        console.error("Failed to fetch config", error)
        setState(previous => ({...previous, capabilitiesLoaded: true}))
      }
    }

    const loadReports = async () => {
      try {
        const response = await fetch("/api/reports", {signal: controller.signal})
        if (!response.ok) throw new Error(`Failed to fetch reports: ${response.status}`)

        const reports = (await response.json()) as TestReport[]
        onConnected()
        setState(previous => ({...previous, reports, reportsLoading: false}))
      } catch (error) {
        if (isAbortError(error)) return

        console.error("Failed to fetch reports", error)
        setState(previous => ({...previous, reportsLoading: false}))
      }
    }

    void loadConfig()
    void loadReports()

    return () => controller.abort()
  }, [onConnected])

  return state
}

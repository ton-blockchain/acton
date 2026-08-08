import {useCallback, useEffect, useState} from "react"

import {fetchStudioEnvironments, type StudioEnvironment} from "../studioApi"

const ENVIRONMENT_POLL_INTERVAL_MS = 1500

export interface StudioEnvironmentsState {
  readonly environments: readonly StudioEnvironment[]
  readonly error?: string
  readonly isLoading: boolean
  readonly removeEnvironment: (environmentId: string) => void
  readonly refresh: () => Promise<void>
  readonly setEnvironment: (environment: StudioEnvironment) => void
}

export function useStudioEnvironments(): StudioEnvironmentsState {
  const [environments, setEnvironments] = useState<StudioEnvironment[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string>()

  const refresh = useCallback(async (signal?: AbortSignal) => {
    const nextEnvironments = await fetchStudioEnvironments(signal)
    setEnvironments(current =>
      JSON.stringify(current) === JSON.stringify(nextEnvironments) ? current : nextEnvironments,
    )
    setError(undefined)
    setIsLoading(false)
  }, [])

  useEffect(() => {
    const controller = new AbortController()
    let pollTimer: ReturnType<typeof globalThis.setTimeout> | undefined

    const poll = async () => {
      try {
        await refresh(controller.signal)
      } catch (refreshError) {
        if (controller.signal.aborted) return
        setError(getErrorMessage(refreshError))
        setIsLoading(false)
      }

      if (!controller.signal.aborted) {
        pollTimer = globalThis.setTimeout(() => void poll(), ENVIRONMENT_POLL_INTERVAL_MS)
      }
    }

    void poll()
    return () => {
      controller.abort()
      if (pollTimer !== undefined) globalThis.clearTimeout(pollTimer)
    }
  }, [refresh])

  const setEnvironment = useCallback((nextEnvironment: StudioEnvironment) => {
    setEnvironments(current => {
      const existingIndex = current.findIndex(environment => environment.id === nextEnvironment.id)
      if (existingIndex === -1) return [nextEnvironment, ...current]

      const next = [...current]
      next[existingIndex] = nextEnvironment
      return next
    })
    setError(undefined)
    setIsLoading(false)
  }, [])

  const removeEnvironment = useCallback((environmentId: string) => {
    setEnvironments(current => current.filter(environment => environment.id !== environmentId))
  }, [])

  return {
    environments,
    error,
    isLoading,
    removeEnvironment,
    refresh: () => refresh(),
    setEnvironment,
  }
}

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}

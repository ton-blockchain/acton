import {useCallback, useEffect, useState} from "react"

import {fetchStudioEnvironments, type StudioEnvironment} from "../studioApi"

const ENVIRONMENT_POLL_INTERVAL_MS = 1500

interface StudioEnvironmentState {
  readonly environment?: StudioEnvironment
  readonly error?: string
  readonly isLoading: boolean
  readonly refresh: () => Promise<void>
  readonly setEnvironment: (environment: StudioEnvironment) => void
}

export function useStudioEnvironment(
  environmentId?: string,
  initialEnvironment?: StudioEnvironment,
): StudioEnvironmentState {
  const [environment, setEnvironmentState] = useState<StudioEnvironment | undefined>(
    initialEnvironment?.id === environmentId ? initialEnvironment : undefined,
  )
  const [isLoading, setIsLoading] = useState(Boolean(environmentId && !environment))
  const [error, setError] = useState<string>()

  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      if (!environmentId) return

      const environments = await fetchStudioEnvironments(signal)
      const nextEnvironment = environments.find(candidate => candidate.id === environmentId)
      if (!nextEnvironment) throw new Error("Virtual environment not found")

      setEnvironmentState(nextEnvironment)
      setError(undefined)
      setIsLoading(false)
    },
    [environmentId],
  )

  useEffect(() => {
    if (!environmentId) {
      setEnvironmentState(undefined)
      setError(undefined)
      setIsLoading(false)
      return
    }

    setEnvironmentState(current =>
      current?.id === environmentId
        ? current
        : initialEnvironment?.id === environmentId
          ? initialEnvironment
          : undefined,
    )
    setIsLoading(true)
    setError(undefined)

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
  }, [environmentId, initialEnvironment, refresh])

  const setEnvironment = useCallback((nextEnvironment: StudioEnvironment) => {
    setEnvironmentState(nextEnvironment)
    setError(undefined)
    setIsLoading(false)
  }, [])

  return {
    environment,
    error,
    isLoading,
    refresh: () => refresh(),
    setEnvironment,
  }
}

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}

import {useCallback, useEffect, useRef, useState} from "react"

import {isAbortError} from "./request"

const RUNNER_HEALTH_POLL_INTERVAL_MS = 1500

export function useRunnerConnection() {
  const [connectionLost, setConnectionLost] = useState(false)
  const hasConnected = useRef(false)

  const markConnected = useCallback(() => {
    hasConnected.current = true
    setConnectionLost(false)
  }, [])

  useEffect(() => {
    let activeController: AbortController | undefined

    const checkConnection = async () => {
      if (activeController !== undefined) return

      const controller = new AbortController()
      activeController = controller
      try {
        const response = await fetch("/api/health", {
          cache: "no-store",
          signal: controller.signal,
        })
        if (!response.ok) {
          throw new Error(`Runner health check failed: ${response.status}`)
        }

        markConnected()
      } catch (error) {
        if (!isAbortError(error) && hasConnected.current) {
          setConnectionLost(true)
        }
      } finally {
        if (activeController === controller) {
          activeController = undefined
        }
      }
    }

    void checkConnection()
    const intervalId = globalThis.setInterval(() => {
      void checkConnection()
    }, RUNNER_HEALTH_POLL_INTERVAL_MS)

    return () => {
      activeController?.abort()
      globalThis.clearInterval(intervalId)
    }
  }, [markConnected])

  return {connectionLost, markConnected}
}

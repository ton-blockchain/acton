import {useCallback, useEffect, useRef, useState} from "react"

import {checkStudioConnection, type StudioConnectionState} from "../studioApi"

const STUDIO_HEALTH_POLL_INTERVAL_MS = 1500

interface StudioConnection {
  readonly connectionLost: boolean
  readonly connectionState: StudioConnectionState
  readonly markConnected: () => void
}

/** Tracks the Studio server independently from page-specific API polling.
 *
 * The overlay is only enabled after the first successful request, so a slow
 * initial page load cannot be mistaken for a connection that was lost.
 */
export function useStudioConnection(): StudioConnection {
  const [connectionState, setConnectionState] = useState<StudioConnectionState>("connecting")
  const hasConnected = useRef(false)

  const markConnected = useCallback(() => {
    hasConnected.current = true
    setConnectionState("connected")
  }, [])

  useEffect(() => {
    let activeController: AbortController | undefined

    const checkConnection = async () => {
      if (activeController !== undefined) return

      const controller = new AbortController()
      activeController = controller

      try {
        await checkStudioConnection(controller.signal)
        markConnected()
      } catch (error) {
        if (!isAbortError(error) && hasConnected.current) {
          setConnectionState("disconnected")
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
    }, STUDIO_HEALTH_POLL_INTERVAL_MS)

    return () => {
      activeController?.abort()
      globalThis.clearInterval(intervalId)
    }
  }, [markConnected])

  return {
    connectionLost: connectionState === "disconnected",
    connectionState,
    markConnected,
  }
}

function isAbortError(error: unknown): boolean {
  return error instanceof DOMException && error.name === "AbortError"
}

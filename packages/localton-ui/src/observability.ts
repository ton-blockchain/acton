import {useEffect, useRef, useState} from "react"
import {useToast} from "@acton/ui"

import type {NetworkView, SessionStatsView, TpsView} from "./types"

const POLL_INTERVAL_MS = 1000

export interface ObservabilityClient {
  /** Removes a retained remote report so an intentionally retired offline node disappears */
  readonly forget: (observerId: string, signal?: AbortSignal) => Promise<void>
  /** Reads the name of the node serving this observability endpoint */
  readonly localNodeName: (signal?: AbortSignal) => Promise<string>
  readonly network: (signal?: AbortSignal) => Promise<NetworkView>
  readonly sessionStats: (
    start: number,
    end: number,
    windowSize: number,
    signal?: AbortSignal,
  ) => Promise<SessionStatsView>
  readonly tps: (signal?: AbortSignal) => Promise<TpsView | undefined>
}

export interface ObservabilitySnapshot {
  readonly network: NetworkView | undefined
  readonly now: number
  readonly tps: TpsView | undefined
}

interface LocalObservation {
  readonly payload: {
    readonly name: string
  }
}

/** Creates a typed client for either the standalone collector or a Studio proxy endpoint */
export function createObservabilityClient(baseUrl = ""): ObservabilityClient {
  const request = async <T>(path: string, signal?: AbortSignal): Promise<T> => {
    const response = await fetch(`${baseUrl.replace(/\/$/, "")}${path}`, {
      cache: "no-store",
      headers: {accept: "application/json"},
      signal,
    })
    if (!response.ok) throw new Error(`Request failed with status ${response.status}`)
    return response.json() as Promise<T>
  }

  return {
    forget: async (observerId, signal) => {
      const response = await fetch(
        `${baseUrl.replace(/\/$/, "")}/api/v1/observations/${encodeURIComponent(observerId)}`,
        {
          method: "DELETE",
          headers: {accept: "application/json"},
          signal,
        },
      )
      if (!response.ok) throw new Error(`Request failed with status ${response.status}`)
    },
    localNodeName: signal =>
      request<LocalObservation>("/api/v1/observation", signal).then(
        observation => observation.payload.name,
      ),
    network: signal => request<NetworkView>("/api/v1/network", signal),
    sessionStats: (start, end, windowSize, signal) => {
      const query = new URLSearchParams({
        start: String(start),
        end: String(end),
        window_size: String(windowSize),
      })
      return request<SessionStatsView>(`/api/v1/stats/session?${query}`, signal)
    },
    tps: signal =>
      request<TpsView>("/api/v1/stats/tps", signal).catch(error => {
        if (error instanceof DOMException && error.name === "AbortError") throw error
        return undefined
      }),
  }
}

/** Reads the stable name of the node that owns the standalone dashboard */
export function useLocalNodeName(client: ObservabilityClient): string | undefined {
  const [nodeName, setNodeName] = useState<string>()

  useEffect(() => {
    const controller = new AbortController()

    client
      .localNodeName(controller.signal)
      .then(name => {
        const normalized = name.trim()
        if (normalized) setNodeName(normalized)
      })
      .catch(() => undefined)

    return () => controller.abort()
  }, [client])

  return nodeName
}

/** Keeps one live whole-network snapshot while avoiding overlapping one-second refreshes */
export function useObservability(client: ObservabilityClient): ObservabilitySnapshot {
  const [network, setNetwork] = useState<NetworkView>()
  const [tps, setTps] = useState<TpsView>()
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000))
  const errorToastId = useRef<string | undefined>(undefined)
  const {dismissToast, showToast} = useToast()

  useEffect(() => {
    const timer = globalThis.setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000)
    return () => globalThis.clearInterval(timer)
  }, [])

  useEffect(() => {
    let active = true
    let timer: ReturnType<typeof setTimeout> | undefined
    let controller: AbortController | undefined

    const load = async () => {
      controller = new AbortController()
      try {
        const [nextNetwork, nextTps] = await Promise.all([
          client.network(controller.signal),
          client.tps(controller.signal),
        ])
        if (!active) return

        setNetwork(nextNetwork)
        if (nextTps !== undefined) setTps(nextTps)
        if (errorToastId.current !== undefined) {
          dismissToast(errorToastId.current)
          errorToastId.current = undefined
        }
      } catch (cause) {
        if (!active || (cause instanceof DOMException && cause.name === "AbortError")) return
        if (errorToastId.current === undefined) {
          errorToastId.current = showToast({
            title: "Unable to refresh network data",
            description:
              cause instanceof Error
                ? cause.message
                : "The observability service could not be reached",
            variant: "error",
          })
        }
      } finally {
        controller = undefined
        if (active) timer = globalThis.setTimeout(load, POLL_INTERVAL_MS)
      }
    }

    void load()
    return () => {
      active = false
      controller?.abort()
      if (timer !== undefined) globalThis.clearTimeout(timer)
    }
  }, [client, dismissToast, showToast])

  return {network, now, tps}
}

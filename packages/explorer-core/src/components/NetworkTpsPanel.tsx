import {Skeleton} from "@acton/ui"
import {useEffect, useState} from "react"
import type {FC} from "react"

import type {LoadNetworkTps, NetworkTpsSnapshot} from "../api/networkStats"

import styles from "./NetworkTpsPanel.module.css"

const REFRESH_INTERVAL_MS = 5000
const tpsFormatter = new Intl.NumberFormat("en-US", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
})
const countFormatter = new Intl.NumberFormat("en-US")

interface NetworkTpsPanelProps {
  readonly loadNetworkTps: LoadNetworkTps
}

export const NetworkTpsPanel: FC<NetworkTpsPanelProps> = ({loadNetworkTps}) => {
  const [snapshot, setSnapshot] = useState<NetworkTpsSnapshot>()
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    let active = true
    let timeoutId: ReturnType<typeof setTimeout> | undefined
    let controller: AbortController | undefined

    const load = async () => {
      controller = new AbortController()
      try {
        const next = await loadNetworkTps(controller.signal)
        if (active) {
          setSnapshot(next)
          setFailed(false)
        }
      } catch (error) {
        if (active && !(error instanceof DOMException && error.name === "AbortError")) {
          setFailed(true)
        }
      } finally {
        if (active) {
          timeoutId = globalThis.setTimeout(() => void load(), REFRESH_INTERVAL_MS)
        }
      }
    }

    void load()
    return () => {
      active = false
      controller?.abort()
      if (timeoutId !== undefined) globalThis.clearTimeout(timeoutId)
    }
  }, [loadNetworkTps])

  if ((!snapshot && failed) || (snapshot && snapshot.latest_masterchain_seqno === undefined)) {
    return null
  }

  return (
    <section className={styles.frame} aria-label="Network TPS">
      <header className={styles.header}>
        <span>Network TPS</span>
        {failed && snapshot ? (
          <span className={styles.meta}>Updates temporarily unavailable</span>
        ) : snapshot?.latest_masterchain_seqno === undefined ? null : (
          <span className={styles.meta}>
            {snapshot.status === "syncing" ? "Syncing" : "Updated"} at masterchain block{" "}
            {countFormatter.format(snapshot.latest_masterchain_seqno)}
          </span>
        )}
      </header>
      {snapshot ? (
        <div className={styles.grid}>
          {snapshot.windows.map(window => (
            <div className={styles.metric} key={window.window_seconds}>
              <span className={styles.period}>{formatPeriod(window.window_seconds)}</span>
              <strong className={styles.value}>{tpsFormatter.format(window.tps)}</strong>
              <span className={styles.detail}>
                {countFormatter.format(window.transactions)} transactions
                {window.complete ? "" : " · partial window"}
              </span>
            </div>
          ))}
        </div>
      ) : (
        <div className={styles.grid} aria-label="Loading network TPS">
          {[60, 300, 900].map(windowSeconds => (
            <div className={styles.metric} key={windowSeconds}>
              <Skeleton width="3.5rem" />
              <Skeleton width="5.5rem" height="1.75rem" />
              <Skeleton width="8rem" />
            </div>
          ))}
        </div>
      )}
    </section>
  )
}

function formatPeriod(seconds: number): string {
  if (seconds % 60 === 0) {
    const minutes = seconds / 60
    return `${minutes} ${minutes === 1 ? "minute" : "minutes"}`
  }
  return `${seconds} seconds`
}

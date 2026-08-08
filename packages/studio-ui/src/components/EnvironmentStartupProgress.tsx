import {Check, LoaderCircle} from "lucide-react"
import {Duration} from "@acton/ui"

import type {EnvironmentStartupTimings} from "../studioApi"

import styles from "./EnvironmentStartupProgress.module.css"

interface EnvironmentStartupProgressProps {
  readonly timings: EnvironmentStartupTimings
}

export function EnvironmentStartupProgress({timings}: EnvironmentStartupProgressProps) {
  const checks = [
    ["TON node", timings.tonReadyMs],
    ["API", timings.apiReadyMs],
    ["Indexer", timings.indexerReadyMs],
  ] as const
  const composeComplete = timings.composeMs !== undefined

  return (
    <section className={styles.root} aria-label="Network startup timings">
      <div className={styles.summary}>
        <div>
          <StatusIcon complete={composeComplete} />
          <span>
            <strong>Docker Compose</strong>
            <small>
              {composeComplete ? "All services are healthy" : "Starting network services"}
            </small>
          </span>
        </div>
        <span className={styles.duration}>
          {composeComplete ? (
            <>
              Completed in{" "}
              <Duration display="startup" unit="milliseconds" value={timings.composeMs} />
            </>
          ) : (
            "Running"
          )}
        </span>
      </div>

      <ol className={styles.checks}>
        {checks.map(([label, duration]) => {
          const complete = duration !== undefined
          return (
            <li key={label} data-state={complete ? "complete" : "active"}>
              <StatusIcon complete={complete} />
              <span>{label}</span>
              <strong>
                {complete ? (
                  <>
                    Ready in <Duration display="startup" unit="milliseconds" value={duration} />
                  </>
                ) : (
                  "Waiting"
                )}
              </strong>
            </li>
          )
        })}
      </ol>
    </section>
  )
}

function StatusIcon({complete}: {readonly complete: boolean}) {
  return (
    <span className={styles.statusIcon} data-state={complete ? "complete" : "active"}>
      {complete ? (
        <Check size={11} aria-hidden="true" />
      ) : (
        <LoaderCircle size={12} aria-hidden="true" />
      )}
    </span>
  )
}

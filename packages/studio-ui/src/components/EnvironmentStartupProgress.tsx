import {Duration, RawDataBlock} from "@acton/ui"
import {Check, LoaderCircle} from "lucide-react"
import {useEffect, useState} from "react"

import {
  fetchStudioEnvironmentStartup,
  type EnvironmentStartupState,
  type EnvironmentStartupTimings,
} from "../studioApi"

import styles from "./EnvironmentStartupProgress.module.css"

interface EnvironmentStartupProgressProps {
  readonly environmentId?: string
  readonly timings: EnvironmentStartupTimings
}

export function EnvironmentStartupProgress({
  environmentId,
  timings,
}: EnvironmentStartupProgressProps) {
  const [startup, setStartup] = useState<EnvironmentStartupState>()
  const [logsError, setLogsError] = useState<string>()
  const checks = [
    ["TON node", timings.tonReadyMs],
    ["API", timings.apiReadyMs],
    ["Indexer", timings.indexerReadyMs],
  ] as const
  const containersComplete = timings.containersMs !== undefined
  const operation = startup?.operation
  const imageComplete =
    operation !== undefined &&
    operation.phase !== "checkingImage" &&
    operation.phase !== "pullingImage" &&
    operation.completedSteps.some(step => step.phase === "checkingImage")
  const imageDuration = operation?.completedSteps
    .filter(step => step.phase === "checkingImage" || step.phase === "pullingImage")
    .reduce((total, step) => total + step.durationMs, 0)

  useEffect(() => {
    if (!environmentId) return

    const controller = new AbortController()
    let polling = false

    const poll = async () => {
      if (polling) return

      polling = true
      try {
        const next = await fetchStudioEnvironmentStartup(environmentId, controller.signal)
        if (controller.signal.aborted) return
        setStartup(next)
        setLogsError(undefined)
      } catch (error) {
        if (!controller.signal.aborted) {
          setLogsError(error instanceof Error ? error.message : String(error))
        }
      } finally {
        polling = false
      }
    }

    void poll()
    const timer = globalThis.setInterval(() => void poll(), 1000)

    return () => {
      controller.abort()
      globalThis.clearInterval(timer)
    }
  }, [environmentId])

  return (
    <div className={styles.container}>
      <section className={styles.root} aria-label="Network startup timings">
        {environmentId ? (
          <div className={styles.summary}>
            <div>
              <StatusIcon complete={imageComplete} />
              <span>
                <strong>Localton image</strong>
                <small>{imageDetail(operation?.phase, operation?.progress?.detail)}</small>
              </span>
            </div>
            <span className={styles.duration}>
              {imageComplete && imageDuration !== undefined ? (
                <>
                  Ready in <Duration display="startup" unit="milliseconds" value={imageDuration} />
                </>
              ) : operation?.phase === "pullingImage" && operation.progress ? (
                `${operation.progress.completed} ${operation.progress.unit}`
              ) : operation?.phase === "checkingImage" ? (
                "Checking"
              ) : (
                "Waiting"
              )}
            </span>
          </div>
        ) : undefined}
        <div className={styles.summary} data-separated={environmentId ? "true" : undefined}>
          <div>
            <StatusIcon complete={containersComplete} />
            <span>
              <strong>Docker services</strong>
              <small>
                {containersComplete ? "All services are healthy" : "Starting network services"}
              </small>
            </span>
          </div>
          <span className={styles.duration}>
            {containersComplete ? (
              <>
                Completed in{" "}
                <Duration display="startup" unit="milliseconds" value={timings.containersMs} />
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
      {environmentId ? (
        <RawDataBlock
          className={styles.logs}
          title="Startup logs"
          titleLabel="startup logs"
          value={startup?.logs ?? logsError ?? ""}
          copyLabel="startup logs"
          collapsible
          defaultExpanded={false}
          empty={!startup?.logs && !logsError}
          emptyContent="Waiting for startup output"
          maxHeight="16rem"
          wrap={false}
        />
      ) : undefined}
    </div>
  )
}

function imageDetail(phase: string | undefined, detail: string | undefined) {
  if (phase === "checkingImage") return "Checking the local Docker cache"
  if (phase === "pullingImage") return detail || "Downloading image layers"
  if (phase) return "Image is ready"
  return "Preparing image check"
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

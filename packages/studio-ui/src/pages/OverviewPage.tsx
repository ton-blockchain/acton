import {DateTime, Duration, formatCountLabel, Skeleton, SourceLocationValue} from "@acton/ui"
import {
  ArrowRight,
  Ban,
  Boxes,
  Check,
  CircleAlert,
  CircleGauge,
  FlaskConical,
  FolderKanban,
  LoaderCircle,
} from "lucide-react"
import {useEffect, useState} from "react"
import type {MouseEvent, ReactNode} from "react"

import {environmentStatusLabels, formatEnvironmentType} from "../environmentPresentation"
import type {
  EnvironmentStatus,
  StudioConnectionState,
  StudioEnvironment,
  TestRunRecord,
  TestRunStatus,
  TestRunSummary,
} from "../studioApi"
import {fetchStudioTestRun} from "../studioApi"
import type {StudioPath} from "../studioPages"
import {testRunStatusLabel, testRunSummary} from "../testRunPresentation"

import styles from "./OverviewPage.module.css"

interface OverviewPageProps {
  readonly connectionState: StudioConnectionState
  readonly environments: readonly StudioEnvironment[]
  readonly environmentsError?: string
  readonly environmentsLoading: boolean
  readonly projectName?: string
  readonly projectPath?: string
  readonly testRuns: readonly TestRunSummary[]
  readonly testRunsError?: string
  readonly testRunsLoading: boolean
  readonly onNavigate: (path: StudioPath) => void
  readonly onOpenEnvironment: (environment: StudioEnvironment) => void
  readonly onSelectTestRun: (runId: string) => void
}

const recentRunCount = 3
const visibleEnvironmentCount = 4
const environmentStatuses: readonly EnvironmentStatus[] = [
  "running",
  "starting",
  "stopping",
  "stopped",
  "failed",
]

export function OverviewPage({
  connectionState,
  environments,
  environmentsError,
  environmentsLoading,
  projectName,
  projectPath,
  testRuns,
  testRunsError,
  testRunsLoading,
  onNavigate,
  onOpenEnvironment,
  onSelectTestRun,
}: OverviewPageProps) {
  const connectionLabel =
    connectionState === "connected"
      ? "Connected"
      : connectionState === "connecting"
        ? "Connecting"
        : "Not connected"
  const connectionDescription =
    connectionState === "connected"
      ? "Studio server is available"
      : connectionState === "connecting"
        ? "Looking for Studio server"
        : "Start Studio with acton studio start"
  const connectionDotClass =
    connectionState === "connected"
      ? styles.statusDotConnected
      : connectionState === "disconnected"
        ? styles.statusDotDisconnected
        : ""
  const workspaceDescription =
    projectPath ??
    (connectionState === "connected"
      ? projectName
        ? "Current project"
        : "No project selected"
      : connectionState === "connecting"
        ? "Connecting to Studio server"
        : "Waiting for Studio server")
  const runningEnvironmentCount = environments.filter(
    environment => environment.status === "running",
  ).length
  const latestRun = testRuns[0]
  const latestRunId = latestRun?.id
  const latestRunStatus = latestRun?.status
  const previousRuns = testRuns.slice(1, recentRunCount + 1)
  const [latestRunDetailsState, setLatestRunDetailsState] = useState<{
    readonly runId: string
    readonly run?: TestRunRecord
  }>()

  useEffect(() => {
    if (latestRunStatus !== "failed" || !latestRunId) return

    const controller = new AbortController()
    void fetchStudioTestRun(latestRunId, controller.signal).then(
      run => {
        if (!controller.signal.aborted) setLatestRunDetailsState({runId: latestRunId, run})
      },
      () => {
        if (!controller.signal.aborted) setLatestRunDetailsState({runId: latestRunId})
      },
    )
    return () => controller.abort()
  }, [latestRunId, latestRunStatus])

  const latestRunDetails =
    latestRunDetailsState?.runId === latestRun?.id ? latestRunDetailsState?.run : undefined
  const latestRunDetailsLoading =
    latestRun?.status === "failed" && latestRunDetailsState?.runId !== latestRun.id

  const navigateFromAnchor = (event: MouseEvent<HTMLAnchorElement>, path: StudioPath) => {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return
    }

    event.preventDefault()
    onNavigate(path)
  }

  return (
    <div className={styles.page}>
      <section className={styles.signalStrip} aria-label="Workspace status">
        <div className={styles.signal}>
          <span className={styles.signalIcon}>
            <FolderKanban size={17} aria-hidden="true" />
          </span>
          <span className={styles.signalCopy}>
            <strong>{projectName || "No project open"}</strong>
            <small className={projectPath ? styles.technicalValue : undefined}>
              {workspaceDescription}
            </small>
          </span>
        </div>
        <div className={styles.signal}>
          <span className={styles.signalIcon}>
            <CircleGauge size={17} aria-hidden="true" />
          </span>
          <span className={styles.signalCopy}>
            <strong className={styles.signalValue}>
              <span className={`${styles.statusDot} ${connectionDotClass}`} />
              {connectionLabel}
            </strong>
            <small>{connectionDescription}</small>
          </span>
        </div>
        <div className={styles.signal}>
          <span className={styles.signalIcon}>
            <Boxes size={17} aria-hidden="true" />
          </span>
          <span className={styles.signalCopy}>
            <strong>
              {environmentsLoading
                ? "Loading"
                : `${runningEnvironmentCount} / ${environments.length}`}
            </strong>
            <small>{environmentsLoading ? "Loading environments" : "Running environments"}</small>
          </span>
        </div>
      </section>

      <div className={styles.dashboardGrid}>
        <section className={styles.panel} aria-labelledby="test-activity-title">
          <div className={styles.sectionHeader}>
            <div>
              <h2 id="test-activity-title">Tests</h2>
              <p>Latest result and actionable failures</p>
            </div>
            <a href="/tests" onClick={event => navigateFromAnchor(event, "/tests")}>
              All runs
              <ArrowRight size={15} aria-hidden="true" />
            </a>
          </div>

          {testRunsLoading ? (
            <LoadingTests />
          ) : testRunsError && testRuns.length === 0 ? (
            <PanelMessage title="Could not load test runs" description={testRunsError} />
          ) : testRuns.length === 0 ? (
            <PanelMessage
              icon={<FlaskConical size={20} aria-hidden="true" />}
              title="No test runs yet"
              description="Run project tests to see results and duration trends"
            />
          ) : (
            <>
              <button
                type="button"
                className={styles.latestRun}
                onClick={() => onSelectTestRun(latestRun.id)}
              >
                <span className={styles.latestRunIcon} data-status={latestRun.status}>
                  <LatestRunStatusIcon status={latestRun.status} />
                </span>
                <span className={styles.latestRunCopy}>
                  <span className={styles.latestRunKicker}>
                    <span data-status={latestRun.status}>
                      {testRunStatusLabel(latestRun.status)}
                    </span>
                    <span>{latestRun.source === "studio" ? "Studio" : "CLI"}</span>
                  </span>
                  <strong>{latestRunHeadline(latestRun)}</strong>
                  <span className={styles.latestRunMeta}>
                    {testRunSummary(latestRun)}
                    <span aria-hidden="true">·</span>
                    {latestRun.stats.durationMs > 0 ? (
                      <Duration
                        display="runtime"
                        unit="milliseconds"
                        value={latestRun.stats.durationMs}
                      />
                    ) : (
                      "In progress"
                    )}
                    <span aria-hidden="true">·</span>
                    <DateTime value={latestRun.startedAt} display="smart" />
                  </span>
                  <code title={latestRun.command.join(" ")}>{latestRun.command.join(" ")}</code>
                </span>
                <ArrowRight size={17} aria-hidden="true" />
              </button>

              <LatestRunOutcome
                run={latestRun}
                details={latestRunDetails}
                detailsLoading={latestRunDetailsLoading}
              />

              {previousRuns.length > 0 ? (
                <>
                  <div className={styles.recentRunsHeader}>
                    <strong>Previous runs</strong>
                    <span>{previousRuns.length}</span>
                  </div>
                  <div className={styles.recentRuns}>
                    {previousRuns.map(run => (
                      <button
                        key={run.id}
                        type="button"
                        className={styles.runRow}
                        onClick={() => onSelectTestRun(run.id)}
                      >
                        <span className={styles.runStatus} data-status={run.status}>
                          {testRunStatusLabel(run.status)}
                        </span>
                        <span className={styles.runDescription}>{testRunSummary(run)}</span>
                        <span className={styles.runDuration}>
                          {run.stats.durationMs > 0 ? (
                            <Duration
                              display="runtime"
                              unit="milliseconds"
                              value={run.stats.durationMs}
                            />
                          ) : (
                            "—"
                          )}
                        </span>
                        <DateTime
                          className={styles.runTime}
                          value={run.startedAt}
                          display="smart"
                        />
                        <ArrowRight size={15} aria-hidden="true" />
                      </button>
                    ))}
                  </div>
                </>
              ) : null}
            </>
          )}
        </section>

        <section className={styles.panel} aria-labelledby="environments-title">
          <div className={styles.sectionHeader}>
            <div>
              <h2 id="environments-title">Environments</h2>
              <p>Workspace network status</p>
            </div>
            <a
              href="/virtual-environments"
              onClick={event => navigateFromAnchor(event, "/virtual-environments")}
            >
              View all
              <ArrowRight size={15} aria-hidden="true" />
            </a>
          </div>

          {environmentsLoading ? (
            <LoadingEnvironments />
          ) : environmentsError && environments.length === 0 ? (
            <PanelMessage title="Could not load environments" description={environmentsError} />
          ) : environments.length === 0 ? (
            <PanelMessage
              icon={<Boxes size={20} aria-hidden="true" />}
              title="No virtual environments"
              description="Create a simulated or full localnet for this workspace"
            />
          ) : (
            <>
              <div className={styles.environmentSummary}>
                <strong>
                  {runningEnvironmentCount}
                  <span> / {environments.length}</span>
                </strong>
                <small>Running now</small>
                <div
                  className={styles.environmentRail}
                  aria-label="Environment status distribution"
                >
                  {environmentStatuses.map(status => {
                    const count = environments.filter(
                      environment => environment.status === status,
                    ).length
                    if (count === 0) return null
                    return (
                      <span
                        key={status}
                        data-status={status}
                        style={{flexGrow: count}}
                        title={`${environmentStatusLabels[status]}: ${count}`}
                      />
                    )
                  })}
                </div>
                <div className={styles.environmentLegend}>
                  {environmentStatuses.map(status => {
                    const count = environments.filter(
                      environment => environment.status === status,
                    ).length
                    if (count === 0) return null
                    return (
                      <span key={status} data-status={status}>
                        {environmentStatusLabels[status]} {count}
                      </span>
                    )
                  })}
                </div>
              </div>

              <div className={styles.environmentList}>
                {environments.slice(0, visibleEnvironmentCount).map(environment => (
                  <button
                    key={environment.id}
                    type="button"
                    className={styles.environmentRow}
                    onClick={() => onOpenEnvironment(environment)}
                  >
                    <span
                      className={styles.environmentStatusDot}
                      data-status={environment.status}
                    />
                    <span className={styles.environmentCopy}>
                      <strong>{environment.name}</strong>
                      <small>{formatEnvironmentMetadata(environment)}</small>
                    </span>
                    <span
                      className={styles.environmentStatusLabel}
                      data-status={environment.status}
                    >
                      {environmentStatusLabels[environment.status]}
                    </span>
                    <ArrowRight size={15} aria-hidden="true" />
                  </button>
                ))}
              </div>
            </>
          )}
        </section>
      </div>
    </div>
  )
}

function LatestRunOutcome({
  details,
  detailsLoading,
  run,
}: {
  readonly details?: TestRunRecord
  readonly detailsLoading: boolean
  readonly run: TestRunSummary
}) {
  if (run.status === "failed") {
    const failures = details?.reports.filter(report => report.status === "Failed").slice(0, 3) ?? []
    const remainingFailures = Math.max(0, run.stats.failed - failures.length)

    return (
      <section className={styles.failures} aria-label="Failures in the latest run">
        <header className={styles.failuresHeader}>
          <strong>Failures</strong>
          {run.stats.failed > 0 ? <span>{run.stats.failed}</span> : null}
        </header>
        {detailsLoading ? (
          <div className={styles.failureLoading} role="status" aria-label="Loading failures">
            <Skeleton width="38%" />
            <Skeleton width="72%" />
          </div>
        ) : failures.length > 0 ? (
          <div className={styles.failureList}>
            {failures.map(report => {
              const message = failureMessage(report)
              return (
                <div
                  key={`${report.file_path}:${report.row}:${report.column}:${report.name}`}
                  className={styles.failureRow}
                >
                  <CircleAlert size={16} aria-hidden="true" />
                  <span className={styles.failureCopy}>
                    <strong>{report.name}</strong>
                    <SourceLocationValue
                      maxSegments={Number.MAX_SAFE_INTEGER}
                      projectRoot={details?.projectRoot}
                      value={{file: report.file_path, line: report.row + 1}}
                    />
                    {message ? <small title={message}>{message}</small> : null}
                  </span>
                </div>
              )
            })}
            {remainingFailures > 0 ? (
              <div className={styles.moreFailures}>
                +{remainingFailures} more {remainingFailures === 1 ? "failure" : "failures"}
              </div>
            ) : null}
          </div>
        ) : (
          <OutcomeNotice
            status="failed"
            title="Run failed before test results were recorded"
            description={run.error ?? "Open the run to inspect runner output and build errors"}
          />
        )}
      </section>
    )
  }

  if (run.status === "passed") {
    return (
      <OutcomeNotice
        status="passed"
        title="No failures in the latest run"
        description={testRunSummary(run)}
      />
    )
  }

  if (run.status === "cancelled") {
    return (
      <OutcomeNotice
        status="cancelled"
        title="Latest run was cancelled"
        description={run.error ?? "Start another run when you are ready"}
      />
    )
  }

  return (
    <OutcomeNotice
      status={run.status}
      title={run.status === "queued" ? "Test run is queued" : "Test run is in progress"}
      description="Results and failures will appear here when the run finishes"
    />
  )
}

function OutcomeNotice({
  description,
  status,
  title,
}: {
  readonly description: string
  readonly status: TestRunStatus
  readonly title: string
}) {
  return (
    <div className={styles.outcomeNotice} data-status={status}>
      <LatestRunStatusIcon status={status} />
      <span>
        <strong>{title}</strong>
        <small>{description}</small>
      </span>
    </div>
  )
}

function LatestRunStatusIcon({status}: {readonly status: TestRunStatus}) {
  if (status === "passed") return <Check size={18} aria-hidden="true" />
  if (status === "failed") return <CircleAlert size={18} aria-hidden="true" />
  if (status === "cancelled") return <Ban size={18} aria-hidden="true" />
  return <LoaderCircle className={styles.spinner} size={18} aria-hidden="true" />
}

function LoadingTests() {
  return (
    <div className={styles.loadingPanel} role="status" aria-label="Loading test activity">
      <div className={styles.loadingLatestRun}>
        <Skeleton shape="rect" width="2.5rem" height="2.5rem" radius="md" />
        <div>
          <Skeleton width="6rem" />
          <Skeleton width="15rem" height="1.5rem" />
          <Skeleton width="21rem" />
        </div>
      </div>
      <Skeleton width="58%" />
      <Skeleton width="100%" height="3.5rem" />
      <Skeleton width="100%" height="3.5rem" />
    </div>
  )
}

function LoadingEnvironments() {
  return (
    <div className={styles.loadingPanel} role="status" aria-label="Loading environments">
      <Skeleton width="7rem" height="2rem" />
      <Skeleton shape="rect" width="100%" height="0.5rem" radius="round" />
      <Skeleton width="100%" height="3.5rem" />
      <Skeleton width="100%" height="3.5rem" />
      <Skeleton width="100%" height="3.5rem" />
    </div>
  )
}

function PanelMessage({
  description,
  icon,
  title,
}: {
  readonly description: string
  readonly icon?: ReactNode
  readonly title: string
}) {
  return (
    <div className={styles.panelMessage}>
      {icon ? <span className={styles.panelMessageIcon}>{icon}</span> : null}
      <strong>{title}</strong>
      <p>{description}</p>
    </div>
  )
}

function formatEnvironmentMetadata(environment: StudioEnvironment) {
  const type = formatEnvironmentType(environment.config)
  return type === environment.network.label ? type : `${type} · ${environment.network.label}`
}

function latestRunHeadline(run: TestRunSummary) {
  if (run.status === "failed") {
    if (run.stats.failed === 1) return "1 test failed"
    if (run.stats.failed > 1) return `${run.stats.failed} tests failed`
    return "Test run failed"
  }
  if (run.status === "passed") {
    return `${formatCountLabel(run.stats.total, {singular: "test"})} passed`
  }
  if (run.status === "cancelled") return "Test run cancelled"
  return run.status === "queued" ? "Waiting to start" : "Running tests"
}

function failureMessage(report: TestRunRecord["reports"][number]) {
  return report.message?.trim() || report.detailed_message?.trim() || report.details?.trim()
}

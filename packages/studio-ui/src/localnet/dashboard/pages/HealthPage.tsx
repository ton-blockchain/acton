import {
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
} from "@acton/ui"
import {Activity, Clock3, Database, Gauge, Server} from "lucide-react"
import {useCallback, useEffect, useState} from "react"
import type {FC} from "react"

import {fetchStudioEnvironmentHealth} from "../../../studioApi"
import type {
  ApiHealth,
  ApiHealthStatus,
  NetworkHealth,
  NetworkHealthSample,
  NetworkHealthStatus,
  ServiceHealth,
  ServiceHealthStatus,
  StudioEnvironment,
} from "../../../studioApi"

import styles from "./HealthPage.module.css"

const POLL_INTERVAL_MS = 2000

interface HealthPageProps {
  readonly environment: StudioEnvironment
}

interface ChartSeries {
  readonly key: keyof NetworkHealthSample
  readonly label: string
  readonly className: string
}

interface MetricChartProps {
  readonly emptyLabel: string
  readonly formatValue: (value: number) => string
  readonly points: readonly NetworkHealthSample[]
  readonly series: readonly ChartSeries[]
  readonly title: string
  readonly wholeNumberScale?: boolean
}

export const HealthPage: FC<HealthPageProps> = ({environment}) => {
  const [health, setHealth] = useState<NetworkHealth>()
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string>()

  const load = useCallback(
    async (signal: AbortSignal) => {
      try {
        const next = await fetchStudioEnvironmentHealth(environment.id, signal)
        setHealth(next)
        setError(undefined)
      } catch (cause) {
        if (!signal.aborted) setError(errorMessage(cause))
      } finally {
        if (!signal.aborted) setLoading(false)
      }
    },
    [environment.id],
  )

  useEffect(() => {
    let active = true
    let controller: AbortController | undefined
    let timer: ReturnType<typeof setTimeout> | undefined

    const poll = async () => {
      controller = new AbortController()
      await load(controller.signal)
      controller = undefined

      if (active) timer = globalThis.setTimeout(poll, POLL_INTERVAL_MS)
    }

    void poll()

    return () => {
      active = false
      controller?.abort()
      if (timer !== undefined) globalThis.clearTimeout(timer)
    }
  }, [load])

  if (!health && loading) {
    return <HealthPageSkeleton />
  }

  if (!health) {
    return (
      <div className={styles.unavailable} role="alert">
        <Activity size={18} aria-hidden="true" />
        <div>
          <strong>Health data unavailable</strong>
          <span>{error ?? "Studio could not inspect this environment"}</span>
        </div>
      </div>
    )
  }

  return (
    <div className={styles.page}>
      {error ? (
        <div className={styles.stale} role="status">
          Showing the last sample because refresh failed: {error}
        </div>
      ) : undefined}

      <section className={styles.summary} aria-label="Full localnet health summary">
        <HealthSummaryCard health={health} />
        <ApiSummaryCard api={health.apiV2} icon={Server} label="API v2" />
        <ApiSummaryCard
          api={health.apiV3}
          detail={formatLag(health.indexerLagBlocks, health.estimatedIndexerLagMs)}
          icon={Database}
          label="API v3"
        />
        <div className={styles.summaryCard}>
          <span className={styles.summaryHeading}>
            <Clock3 size={16} aria-hidden="true" />
            Latest block
          </span>
          <strong>{formatAge(health.apiV2.blockAgeMs)}</strong>
          <span className={styles.summaryDetail}>
            {health.apiV2.blockTimeUnix === null
              ? "Block timestamp unavailable"
              : formatTimestamp(health.apiV2.blockTimeUnix)}
          </span>
        </div>
      </section>

      {health.infrastructureError ? (
        <div className={styles.infrastructureError} role="alert">
          <strong>Docker state unavailable</strong>
          <span>{health.infrastructureError}</span>
        </div>
      ) : undefined}

      <section className={styles.charts} aria-label="Health history">
        <MetricChart
          emptyLabel="Collecting API latency samples"
          formatValue={formatMilliseconds}
          points={health.history}
          series={[
            {key: "apiV2LatencyMs", label: "API v2", className: styles.lineV2},
            {key: "apiV3LatencyMs", label: "API v3", className: styles.lineV3},
          ]}
          title="Probe latency"
        />
        <MetricChart
          emptyLabel="Collecting indexer lag samples"
          formatValue={formatBlocks}
          points={health.history}
          series={[{key: "indexerLagBlocks", label: "Indexer lag", className: styles.lineLag}]}
          title="Indexer lag"
          wholeNumberScale
        />
      </section>

      <ServicesTable services={health.services} />
    </div>
  )
}

const HealthSummaryCard: FC<{readonly health: NetworkHealth}> = ({health}) => (
  <div className={styles.summaryCard}>
    <span className={styles.summaryHeading}>
      <Gauge size={16} aria-hidden="true" />
      Full localnet
    </span>
    <strong className={styles.statusValue} data-status={health.status}>
      <span className={styles.statusDot} aria-hidden="true" />
      {networkStatusLabel(health.status)}
    </strong>
    <span className={styles.summaryDetail}>Updated {formatSampleAge(health.observedAtMs)}</span>
  </div>
)

interface ApiSummaryCardProps {
  readonly api: ApiHealth
  readonly detail?: string
  readonly icon: typeof Server
  readonly label: string
}

const ApiSummaryCard: FC<ApiSummaryCardProps> = ({api, detail, icon: Icon, label}) => (
  <div className={styles.summaryCard}>
    <span className={styles.summaryHeading}>
      <Icon size={16} aria-hidden="true" />
      {label}
    </span>
    <strong className={styles.statusValue} data-status={api.status}>
      <span className={styles.statusDot} aria-hidden="true" />
      {apiStatusLabel(api.status)}
    </strong>
    <span className={styles.summaryDetail}>
      {api.masterchainSeqno === null ? "No indexed head" : `Block #${api.masterchainSeqno}`}
      {api.latencyMs === null ? "" : ` · ${formatMilliseconds(api.latencyMs)}`}
      {detail ? ` · ${detail}` : ""}
    </span>
    {api.error ? <span className={styles.apiError}>{api.error}</span> : undefined}
  </div>
)

const MetricChart: FC<MetricChartProps> = ({
  emptyLabel,
  formatValue,
  points,
  series,
  title,
  wholeNumberScale = false,
}) => {
  const values = points.flatMap(point =>
    series.flatMap(item => {
      const value = point[item.key]
      return typeof value === "number" ? [value] : []
    }),
  )
  const hasHistory = points.length > 1 && values.length > 1
  const maximum = Math.max(1, ...values)
  const minimumTime = points[0]?.observedAtMs ?? 0
  const maximumTime = points.at(-1)?.observedAtMs ?? minimumTime + 1
  const timeRange = Math.max(1, maximumTime - minimumTime)
  const x = (time: number) => 64 + ((time - minimumTime) / timeRange) * 696
  const y = (value: number) => 184 - (value / maximum) * 148
  const tickValues = wholeNumberScale
    ? [...new Set([0, Math.round(maximum / 2), Math.round(maximum)])]
    : [0, maximum / 2, maximum]

  return (
    <article className={styles.chartPanel}>
      <header className={styles.chartHeader}>
        <div>
          <strong>{title}</strong>
          <span>{hasHistory ? `${points.length} recent samples` : emptyLabel}</span>
        </div>
        <div className={styles.legend}>
          {series.map(item => (
            <span key={item.label}>
              <i className={item.className} aria-hidden="true" />
              {item.label}
            </span>
          ))}
        </div>
      </header>

      {hasHistory ? (
        <div className={styles.chartBody}>
          <svg viewBox="0 0 800 228" role="img" aria-label={`${title} over recent samples`}>
            {tickValues.map(value => {
              const gridY = y(value)

              return (
                <g key={value}>
                  <line className={styles.gridLine} x1="64" x2="760" y1={gridY} y2={gridY} />
                  <text className={styles.axisLabel} x="56" y={gridY + 4} textAnchor="end">
                    {formatValue(value)}
                  </text>
                </g>
              )
            })}
            {series.map(item =>
              chartSegments(points, item.key, x, y).map((segment, index) => (
                <polyline
                  key={`${item.label}-${index}`}
                  className={`${styles.chartLine} ${item.className}`}
                  points={segment}
                />
              )),
            )}
            <text className={styles.timeLabel} x="64" y="216">
              {formatChartTime(minimumTime)}
            </text>
            <text className={styles.timeLabel} x="760" y="216" textAnchor="end">
              {formatChartTime(maximumTime)}
            </text>
          </svg>
        </div>
      ) : (
        <div className={styles.chartEmpty}>{emptyLabel}</div>
      )}
    </article>
  )
}

const ServicesTable: FC<{readonly services: readonly ServiceHealth[]}> = ({services}) => (
  <DataTable title="Services" minWidth="44rem">
    <DataTableTable aria-label="Full localnet services">
      <DataTableHead>
        <DataTableRow>
          <DataTableHeaderCell columnWidth="28%">Service</DataTableHeaderCell>
          <DataTableHeaderCell columnWidth="29%">Role</DataTableHeaderCell>
          <DataTableHeaderCell columnWidth="20%">Status</DataTableHeaderCell>
          <DataTableHeaderCell align="right">Docker state</DataTableHeaderCell>
        </DataTableRow>
      </DataTableHead>
      <DataTableBody>
        {services.length === 0 ? (
          <DataTableRow>
            <DataTableCell>—</DataTableCell>
            <DataTableCell>No Compose services observed</DataTableCell>
            <DataTableCell>—</DataTableCell>
            <DataTableCell align="right">—</DataTableCell>
          </DataTableRow>
        ) : (
          services.map(service => (
            <DataTableRow key={service.name}>
              <DataTableCell>
                <span className={styles.serviceName}>{serviceLabel(service.name)}</span>
              </DataTableCell>
              <DataTableCell>{serviceRole(service.name)}</DataTableCell>
              <DataTableCell>
                <span className={styles.serviceStatus} data-status={service.status}>
                  <span className={styles.statusDot} aria-hidden="true" />
                  {serviceStatusLabel(service.status)}
                </span>
              </DataTableCell>
              <DataTableCell align="right">
                <span className={styles.dockerState}>{dockerState(service)}</span>
              </DataTableCell>
            </DataTableRow>
          ))
        )}
      </DataTableBody>
    </DataTableTable>
  </DataTable>
)

const HealthPageSkeleton: FC = () => (
  <div className={styles.page} aria-label="Loading health data">
    <section className={styles.summary}>
      {Array.from({length: 4}, (_, index) => (
        <div className={styles.summaryCard} key={index}>
          <span className={styles.skeletonWide} />
          <span className={styles.skeletonValue} />
          <span className={styles.skeletonDetail} />
        </div>
      ))}
    </section>
    <DataTable title="Services" minWidth="44rem">
      <DataTableTable aria-label="Loading Full localnet services">
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell>Service</DataTableHeaderCell>
            <DataTableHeaderCell>Role</DataTableHeaderCell>
            <DataTableHeaderCell>Status</DataTableHeaderCell>
            <DataTableHeaderCell align="right">Docker state</DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          <DataTableSkeletonRows columns={4} rows={6} />
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  </div>
)

function chartSegments(
  points: readonly NetworkHealthSample[],
  key: keyof NetworkHealthSample,
  x: (value: number) => number,
  y: (value: number) => number,
): string[] {
  const segments: string[][] = []
  let current: string[] = []

  for (const point of points) {
    const value = point[key]

    if (typeof value !== "number") {
      if (current.length > 0) segments.push(current)
      current = []
      continue
    }

    current.push(`${x(point.observedAtMs).toFixed(1)},${y(value).toFixed(1)}`)
  }

  if (current.length > 0) segments.push(current)

  return segments.map(segment => segment.join(" "))
}

function networkStatusLabel(status: NetworkHealthStatus): string {
  switch (status) {
    case "healthy":
      return "Healthy"
    case "syncing":
      return "Syncing"
    case "degraded":
      return "Degraded"
    case "stopped":
      return "Stopped"
    default:
      return "Unknown"
  }
}

function apiStatusLabel(status: ApiHealthStatus): string {
  switch (status) {
    case "ready":
      return "Ready"
    case "syncing":
      return "Syncing"
    case "unavailable":
      return "Unavailable"
    case "stopped":
      return "Stopped"
    default:
      return "Unknown"
  }
}

function serviceStatusLabel(status: ServiceHealthStatus): string {
  switch (status) {
    case "ready":
      return "Ready"
    case "starting":
      return "Starting"
    case "completed":
      return "Completed"
    case "stopped":
      return "Stopped"
    case "failed":
      return "Failed"
    case "unknown":
      return "Unknown"
    default:
      return "Unknown"
  }
}

function serviceLabel(name: string): string {
  const labels: Readonly<Record<string, string>> = {
    localton: "Localton",
    postgres: "PostgreSQL",
    redis: "Redis",
    "v3-basechain-bootstrap": "Basechain bootstrap",
    "v3-migrations": "V3 migrations",
    "v3-worker": "V3 worker",
    "v3-account-scanner": "Account scanner",
    "v3-api": "API v3",
    "v3-classifier": "Action classifier",
  }

  return labels[name] ?? name
}

function serviceRole(name: string): string {
  const roles: Readonly<Record<string, string>> = {
    localton: "Network runtime and process supervisor",
    postgres: "Indexed chain storage",
    redis: "API cache and classifier queue",
    "v3-basechain-bootstrap": "Initial index boundary",
    "v3-migrations": "Database schema",
    "v3-worker": "Block and transaction indexing",
    "v3-account-scanner": "Initial account indexing",
    "v3-api": "TON Center API v3",
    "v3-classifier": "Action classification",
  }

  return roles[name] ?? "Additional network node"
}

function dockerState(service: ServiceHealth): string {
  const parts = [service.state, service.health].filter(Boolean)

  if (service.exitCode !== null && service.exitCode !== 0) {
    parts.push(`exit ${service.exitCode}`)
  }

  return parts.length > 0 ? parts.join(" · ") : "Not created"
}

function formatLag(blocks: number | null, milliseconds: number | null): string {
  if (blocks === null) return "Lag unavailable"
  if (blocks === 0) return "Indexer caught up"

  const duration = milliseconds === null ? "" : ` · about ${formatDuration(milliseconds)}`
  return `${blocks} ${blocks === 1 ? "block" : "blocks"} behind${duration}`
}

function formatAge(milliseconds: number | null): string {
  if (milliseconds === null) return "—"
  if (milliseconds < 1000) return "Now"
  return `${formatDuration(milliseconds)} ago`
}

function formatDuration(milliseconds: number): string {
  if (milliseconds < 1000) return `${Math.round(milliseconds)} ms`
  if (milliseconds < 60_000) return `${(milliseconds / 1000).toFixed(1)} s`
  return `${(milliseconds / 60_000).toFixed(1)} min`
}

function formatMilliseconds(value: number): string {
  return `${Math.round(value)} ms`
}

function formatBlocks(value: number): string {
  const blocks = Number.isInteger(value) ? value.toFixed(0) : value.toFixed(1)
  return `${blocks} ${value === 1 ? "block" : "blocks"}`
}

function formatTimestamp(unixTime: number): string {
  return new Date(unixTime * 1000).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
}

function formatChartTime(milliseconds: number): string {
  return new Date(milliseconds).toLocaleTimeString([], {hour: "2-digit", minute: "2-digit"})
}

function formatSampleAge(observedAtMs: number): string {
  const seconds = Math.max(0, Math.round((Date.now() - observedAtMs) / 1000))
  return seconds < 2 ? "just now" : `${seconds} seconds ago`
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Studio could not inspect this environment"
}

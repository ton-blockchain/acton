import type {ObservabilityClient} from "../observability"
import type {SessionStatBucket, SessionStatsView} from "../types"
import {Button, formatDuration, useTheme} from "@acton/ui"
import {LineChart} from "echarts/charts"
import {
  AriaComponent,
  DataZoomComponent,
  GridComponent,
  LegendComponent,
  TooltipComponent,
} from "echarts/components"
import {init, use, type EChartsType} from "echarts/core"
import {CanvasRenderer} from "echarts/renderers"
import {ChevronDown, ChevronRight, RefreshCw} from "lucide-react"
import {useCallback, useEffect, useMemo, useRef, useState} from "react"

import styles from "./SessionStats.module.css"

type ChartKind = "lines" | "stacked"
type ChartUnit =
  | "blocks/s"
  | "bytes"
  | "cells"
  | "count"
  | "messages"
  | "percent"
  | "seconds"
  | "shards"
  | "tx"
  | "tx/s"
type ValueMode = "avg" | "rate" | "sum" | "min" | "max"

interface SessionStatsProps {
  readonly client: ObservabilityClient
}

interface MetricDefinition {
  readonly label: string
  readonly mode: ValueMode
  readonly stat: string
  readonly workchain: number
}

interface ChartDefinition {
  readonly kind: ChartKind
  readonly metrics: readonly MetricDefinition[]
  readonly title: string
  readonly unit: ChartUnit
}

interface TimeRange {
  readonly from: number
  readonly to: number
}

const RECENT_RANGES = [
  {label: "1h", seconds: 60 * 60},
  {label: "2h", seconds: 2 * 60 * 60},
  {label: "6h", seconds: 6 * 60 * 60},
  {label: "1d", seconds: 24 * 60 * 60},
  {label: "1w", seconds: 7 * 24 * 60 * 60},
] as const

const WINDOWS = [
  {label: "1m", seconds: 60},
  {label: "5m", seconds: 5 * 60},
  {label: "15m", seconds: 15 * 60},
  {label: "1h", seconds: 60 * 60},
] as const

const CHART_COLORS = [
  "#4f8cff",
  "#22a184",
  "#d69e2e",
  "#d06475",
  "#8b6fd9",
  "#3ba7b8",
  "#ba7448",
  "#6f8e52",
  "#b45a9b",
  "#5f78ad",
  "#a58b44",
  "#4d9b72",
  "#b66c55",
  "#7779c8",
  "#5c93a0",
  "#9b6d8f",
  "#8a845a",
  "#4f8a92",
  "#a36b65",
  "#6a7d9c",
]

use([
  AriaComponent,
  CanvasRenderer,
  DataZoomComponent,
  GridComponent,
  LegendComponent,
  LineChart,
  TooltipComponent,
])

/** Renders the MLT Session Stats chart set from one Localton observability endpoint */
export function SessionStats({client}: SessionStatsProps) {
  const [recentSeconds, setRecentSeconds] = useState(2 * 60 * 60)
  const [windowSeconds, setWindowSeconds] = useState(60)
  const [snapshot, setSnapshot] = useState<SessionStatsView>()
  const [visibleRange, setVisibleRange] = useState<TimeRange>()
  const [error, setError] = useState<string>()
  const [refreshing, setRefreshing] = useState(false)

  const load = useCallback(
    async (signal?: AbortSignal) => {
      setRefreshing(true)
      const end = Math.floor(Date.now() / 1000)
      const start = end - recentSeconds

      try {
        const next = await client.sessionStats(start, end, windowSeconds, signal)
        setSnapshot(next)
        setVisibleRange({from: start, to: end})
        setError(undefined)
      } catch (cause) {
        if (cause instanceof DOMException && cause.name === "AbortError") return
        setError(cause instanceof Error ? cause.message : "Unable to read validator session stats")
      } finally {
        if (!signal?.aborted) setRefreshing(false)
      }
    },
    [client, recentSeconds, windowSeconds],
  )

  useEffect(() => {
    const controller = new AbortController()
    void load(controller.signal)
    const timer = globalThis.setInterval(() => void load(controller.signal), 60_000)

    return () => {
      controller.abort()
      globalThis.clearInterval(timer)
    }
  }, [load])

  const charts = useMemo(() => chartDefinitions(snapshot?.sources ?? []), [snapshot?.sources])

  return (
    <div className={styles.page}>
      <div className={styles.toolbar}>
        <ControlGroup
          label="Range"
          options={RECENT_RANGES}
          selected={recentSeconds}
          onSelect={setRecentSeconds}
        />
        <ControlGroup
          label="Window"
          options={WINDOWS}
          selected={windowSeconds}
          onSelect={setWindowSeconds}
        />
        <div className={styles.status} aria-live="polite">
          {snapshot?.indexed_to ? (
            <span>{`Indexed through ${formatTime(snapshot.indexed_to)} · ${formatWindow(snapshot.bucket_seconds)} buckets`}</span>
          ) : (
            <span>Indexing validator session logs</span>
          )}
          <Button
            aria-label="Refresh session stats"
            title="Refresh"
            size="icon"
            variant="ghost"
            disabled={refreshing}
            onClick={() => void load()}
          >
            <RefreshCw className={refreshing ? styles.spinning : undefined} size={16} />
          </Button>
        </div>
      </div>

      {error ? <div className={styles.error}>{error}</div> : undefined}
      {!snapshot || snapshot.status === "indexing" ? (
        <div className={styles.emptyState}>
          <span className={styles.pulse} aria-hidden="true" />
          Waiting for complete validator sessions
        </div>
      ) : (
        <div className={styles.chartList}>
          {charts.map(chart => (
            <ChartPanel
              key={chart.title}
              buckets={snapshot.buckets}
              bucketSeconds={snapshot.bucket_seconds}
              chart={chart}
              range={visibleRange}
              onRangeChange={setVisibleRange}
            />
          ))}
        </div>
      )}
    </div>
  )
}

function ControlGroup({
  label,
  onSelect,
  options,
  selected,
}: {
  readonly label: string
  readonly onSelect: (seconds: number) => void
  readonly options: readonly {readonly label: string; readonly seconds: number}[]
  readonly selected: number
}) {
  return (
    <div className={styles.controlGroup}>
      <span>{label}</span>
      <div className={styles.buttonGroup}>
        {options.map(option => (
          <Button
            key={option.seconds}
            size="sm"
            variant={selected === option.seconds ? "secondary" : "ghost"}
            aria-pressed={selected === option.seconds}
            onClick={() => onSelect(option.seconds)}
          >
            {option.label}
          </Button>
        ))}
      </div>
    </div>
  )
}

function ChartPanel({
  bucketSeconds,
  buckets,
  chart,
  onRangeChange,
  range,
}: {
  readonly bucketSeconds: number
  readonly buckets: readonly SessionStatBucket[]
  readonly chart: ChartDefinition
  readonly onRangeChange: (range: TimeRange) => void
  readonly range: TimeRange | undefined
}) {
  const [open, setOpen] = useState(true)

  return (
    <section className={styles.chartPanel}>
      <button
        type="button"
        className={styles.chartHeader}
        aria-expanded={open}
        onClick={() => setOpen(current => !current)}
      >
        {open ? <ChevronDown size={17} /> : <ChevronRight size={17} />}
        <span>{chart.title}</span>
      </button>
      {open ? (
        <SessionChart
          bucketSeconds={bucketSeconds}
          buckets={buckets}
          chart={chart}
          range={range}
          onRangeChange={onRangeChange}
        />
      ) : undefined}
    </section>
  )
}

function SessionChart({
  bucketSeconds,
  buckets,
  chart,
  onRangeChange,
  range,
}: {
  readonly bucketSeconds: number
  readonly buckets: readonly SessionStatBucket[]
  readonly chart: ChartDefinition
  readonly onRangeChange: (range: TimeRange) => void
  readonly range: TimeRange | undefined
}) {
  const elementRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<EChartsType | undefined>(undefined)
  const extentRef = useRef<TimeRange>({from: 0, to: 0})
  const rangeCallbackRef = useRef(onRangeChange)
  const {theme} = useTheme()
  const hasDenseLegend = chart.metrics.length > 8

  useEffect(() => {
    rangeCallbackRef.current = onRangeChange
  }, [onRangeChange])

  useEffect(() => {
    const element = elementRef.current
    if (!element) return

    const instance = init(element, undefined, {renderer: "canvas"})
    chartRef.current = instance
    const resizeObserver = new ResizeObserver(() => instance.resize())
    resizeObserver.observe(element)

    instance.on("datazoom", rawEvent => {
      const event = rawEvent as {
        readonly batch?: readonly {readonly start?: number; readonly end?: number}[]
        readonly start?: number
        readonly end?: number
      }
      const payload = event.batch?.[0] ?? event
      const start = Number(payload.start ?? 0)
      const end = Number(payload.end ?? 100)
      const extent = extentRef.current
      const duration = extent.to - extent.from

      rangeCallbackRef.current({
        from: extent.from + (duration * start) / 100,
        to: extent.from + (duration * end) / 100,
      })
    })

    return () => {
      resizeObserver.disconnect()
      instance.dispose()
      chartRef.current = undefined
    }
  }, [])

  useEffect(() => {
    const instance = chartRef.current
    const element = elementRef.current
    if (!instance || !element || !range) return

    const computed = getComputedStyle(element)
    const color = (name: string) => computed.getPropertyValue(name).trim()
    const extent = dataExtent(buckets, range)
    extentRef.current = extent
    const startPercent = percentWithin(range.from, extent)
    const endPercent = percentWithin(range.to, extent)

    instance.setOption(
      {
        animation: false,
        aria: {enabled: true, description: chart.title},
        color: CHART_COLORS,
        dataZoom: [
          {
            end: endPercent,
            filterMode: "filter",
            moveOnMouseMove: true,
            moveOnMouseWheel: "shift",
            start: startPercent,
            throttle: 30,
            type: "inside",
            zoomOnMouseWheel: "ctrl",
          },
          {
            backgroundColor: "transparent",
            borderColor: color("--acton-color-border"),
            bottom: 8,
            brushSelect: false,
            end: endPercent,
            fillerColor: color("--session-chart-selection"),
            handleSize: "75%",
            height: 20,
            showDetail: false,
            start: startPercent,
            textStyle: {color: color("--acton-color-text-subtle"), fontSize: 10},
            type: "slider",
          },
        ],
        grid: {
          bottom: 56,
          containLabel: true,
          left: 8,
          right: 18,
          top: hasDenseLegend ? 118 : chart.metrics.length > 4 ? 76 : 56,
        },
        legend: {
          icon: "roundRect",
          itemGap: 12,
          itemHeight: 3,
          itemWidth: 15,
          left: 16,
          right: 16,
          top: 10,
          type: "plain",
          textStyle: {color: color("--acton-color-text-muted"), fontSize: 11},
        },
        series: chart.metrics.map((metric, index) => ({
          areaStyle: chart.kind === "stacked" ? {opacity: 0.15} : undefined,
          data: metricPoints(metric, chart.metrics, buckets, bucketSeconds),
          emphasis: {focus: "series"},
          lineStyle: {
            opacity: metric.mode === "min" || metric.mode === "max" ? 0.65 : 1,
            type: metric.mode === "min" || metric.mode === "max" ? "dashed" : "solid",
            width: chart.kind === "stacked" ? 1.25 : 1.75,
          },
          name: metric.label,
          showSymbol: false,
          stack: chart.kind === "stacked" ? "total" : undefined,
          type: "line",
          z: CHART_COLORS.length - index,
        })),
        tooltip: {
          axisPointer: {label: {show: false}, type: "line"},
          backgroundColor: color("--acton-color-surface-raised"),
          borderColor: color("--acton-color-border-strong"),
          borderWidth: 1,
          confine: true,
          enterable: true,
          formatter: (params: unknown) => formatTooltip(params, chart.unit),
          extraCssText: "box-shadow: 0 12px 32px rgb(0 0 0 / 28%);",
          textStyle: {color: color("--acton-color-text"), fontSize: 12},
          trigger: "axis",
        },
        xAxis: {
          axisLabel: {
            color: color("--acton-color-text-subtle"),
            fontSize: 11,
            formatter: (value: number) => formatTime(value / 1000),
            hideOverlap: true,
          },
          axisLine: {lineStyle: {color: color("--acton-color-border")}},
          axisTick: {show: false},
          min: extent.from * 1000,
          max: extent.to * 1000,
          splitLine: {lineStyle: {color: color("--acton-color-border"), type: "dashed"}},
          type: "time",
        },
        yAxis: {
          axisLabel: {
            color: color("--acton-color-text-subtle"),
            fontSize: 11,
            formatter: (value: number) => formatChartValue(value, chart.unit, false),
          },
          axisLine: {show: false},
          axisTick: {show: false},
          min: 0,
          splitLine: {lineStyle: {color: color("--acton-color-border"), type: "dashed"}},
          type: "value",
        },
      },
      {notMerge: true, silent: true},
    )
  }, [bucketSeconds, buckets, chart, range, theme])

  return (
    <div
      ref={elementRef}
      className={`${styles.chart} ${hasDenseLegend ? styles.chartDense : ""}`}
      role="img"
      aria-label={chart.title}
    />
  )
}

function metricPoints(
  metric: MetricDefinition,
  metrics: readonly MetricDefinition[],
  buckets: readonly SessionStatBucket[],
  bucketSeconds: number,
): [number, number][] {
  const relevant = buckets.filter(bucket =>
    metrics.some(item => item.stat === bucket.stat && item.workchain === bucket.workchain),
  )
  const timestamps = [...new Set(relevant.map(bucket => bucket.timestamp))].sort((a, b) => a - b)
  const withGaps: number[] = []

  for (const timestamp of timestamps) {
    const previous = withGaps.at(-1)
    if (previous !== undefined && timestamp > previous + bucketSeconds) {
      withGaps.push(previous + bucketSeconds)
      if (timestamp > previous + 2 * bucketSeconds) withGaps.push(timestamp - bucketSeconds)
    }
    withGaps.push(timestamp)
  }

  const values = new Map(
    buckets
      .filter(bucket => bucket.stat === metric.stat && bucket.workchain === metric.workchain)
      .map(bucket => [bucket.timestamp, bucketValue(bucket, metric.mode, bucketSeconds)]),
  )

  return withGaps.map(timestamp => [timestamp * 1000, values.get(timestamp) ?? 0])
}

function bucketValue(bucket: SessionStatBucket, mode: ValueMode, bucketSeconds: number): number {
  if (mode === "min") return bucket.min
  if (mode === "max") return bucket.max
  if (mode === "sum") return bucket.sum
  if (mode === "rate") return bucket.sum / bucketSeconds
  return bucket.count > 0 ? bucket.sum / bucket.count : 0
}

function dataExtent(buckets: readonly SessionStatBucket[], fallback: TimeRange): TimeRange {
  if (buckets.length === 0) return fallback

  return {
    from: Math.min(fallback.from, buckets[0].timestamp),
    to: Math.max(fallback.to, buckets.at(-1)?.timestamp ?? fallback.to),
  }
}

function percentWithin(value: number, extent: TimeRange): number {
  if (extent.to <= extent.from) return 0
  return Math.max(0, Math.min(100, ((value - extent.from) / (extent.to - extent.from)) * 100))
}

function formatTooltip(rawParams: unknown, unit: ChartUnit): string {
  const params = (Array.isArray(rawParams) ? rawParams : [rawParams]) as {
    readonly color?: string
    readonly marker?: string
    readonly seriesName?: string
    readonly value?: readonly [number, number]
  }[]
  const timestamp = params[0]?.value?.[0]
  const rows = params
    .filter(item => item.value !== undefined)
    .map(
      item =>
        `<div style="display:flex;align-items:center;gap:14px;justify-content:space-between;min-width:0"><span style="min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${item.marker ?? ""}${item.seriesName ?? ""}</span><strong>${formatChartValue(item.value?.[1] ?? 0, unit, true)}</strong></div>`,
    )
    .join("")

  return `<div style="display:grid;gap:8px;min-width:420px;max-width:calc(100vw - 40px)"><strong>${timestamp ? new Date(timestamp).toLocaleString() : ""}</strong><div style="display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:5px 18px;max-height:224px;overflow:auto;padding-right:4px">${rows}</div></div>`
}

function formatNumber(value: number): string {
  return new Intl.NumberFormat(undefined, {maximumFractionDigits: 3}).format(value)
}

function formatChartValue(value: number, unit: ChartUnit, precise: boolean): string {
  if (unit === "seconds") return formatDuration(value, {display: "precise", unit: "seconds"})
  if (unit === "percent") {
    const percent = value * 100
    return `${precise ? String(percent) : formatNumber(percent)}%`
  }

  const formatted = precise ? String(value) : formatNumber(value)
  return `${formatted} ${unit}`
}

function formatTime(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleTimeString([], {hour: "2-digit", minute: "2-digit"})
}

function formatWindow(seconds: number): string {
  if (seconds % 3600 === 0) return `${seconds / 3600}h`
  if (seconds % 60 === 0) return `${seconds / 60}m`
  return `${seconds}s`
}

function chartDefinitions(sources: readonly string[]): ChartDefinition[] {
  const charts: ChartDefinition[] = [
    singleChart("Blocks per second", "BLOCK_APPLIED_blocks", false, "rate", undefined, "blocks/s"),
    singleChart(
      "Transactions per second",
      "BLOCK_APPLIED_transactions",
      false,
      "rate",
      undefined,
      "tx/s",
    ),
    lineChart(
      "Validate: actual work time (shards)",
      0,
      ["BLOCK_validate_actual_work_time_parallel", "BLOCK_validate_actual_work_time_singlethread"],
      ["Parallel", "Single thread"],
      "seconds",
    ),
    singleChart("Block size", "BLOCK_size", true, "avg", undefined, "bytes"),
    singleChart("Collated data size", "BLOCK_collated_data_size", true, "avg", undefined, "bytes"),
    singleChart("Block size estimate", "BLOCK_size_est", true, "avg", undefined, "bytes"),
    singleChart(
      "Collated data size estimate",
      "BLOCK_collated_data_size_est",
      true,
      "avg",
      undefined,
      "bytes",
    ),
    singleChart("Tx per block", "BLOCK_transactions", true, "avg", undefined, "tx"),
    singleChart("Shards", "shards_count", false, "avg", -1, "shards"),
    singleChart(
      "Shard->Master latency (accepted shard -> accepted master)",
      "BLOCK_APPLIED_shard_latency_a2a",
      true,
      "avg",
      0,
      "seconds",
    ),
    singleChart(
      "Master->Shard latency (accepted master -> accepted shard)",
      "BLOCK_APPLIED_master_latency_a2a",
      true,
      "avg",
      0,
      "seconds",
    ),
    singleChart(
      "Msg queue size per shard",
      "BLOCK_msg_queue_size",
      true,
      "avg",
      undefined,
      "messages",
    ),
    singleChart(
      "Msg queue cleaned per block",
      "BLOCK_msg_queue_cleaned",
      true,
      "avg",
      undefined,
      "messages",
    ),
    singleChart(
      "Queue total processed per block",
      "BLOCK_queue_total_processed",
      true,
      "avg",
      undefined,
      "messages",
    ),
    singleChart(
      "Queue total skipped per block",
      "BLOCK_queue_total_skipped",
      true,
      "avg",
      undefined,
      "messages",
    ),
    singleChart(
      "Queue limit reached fraction",
      "BLOCK_queue_limit_reached",
      false,
      "avg",
      undefined,
      "percent",
    ),
    lineChart(
      "Load fractions (shards)",
      0,
      [
        "BLOCK_load_fraction_queue_cleanup",
        "BLOCK_load_fraction_dispatch",
        "BLOCK_load_fraction_internals",
        "BLOCK_load_fraction_externals",
        "BLOCK_load_fraction_new_msgs",
      ],
      ["Queue cleanup", "Dispatch", "Internals", "Externals (medium limit)", "New msgs"],
      "percent",
    ),
    singleChart(
      "Msg limit from neighbor queue (shards)",
      "BLOCK_neighbor_msg_limit",
      true,
      "avg",
      0,
      "messages",
    ),
    singleChart(
      "Max processed messages from one neighbor (shards)",
      "BLOCK_max_neighbor_processed",
      true,
      "avg",
      0,
      "messages",
    ),
  ]

  for (const workchain of [-1, 0]) {
    for (const time of ["real", "cpu"] as const) {
      const fields = [
        "preinit",
        "queue_cleanup",
        "prelim_storage_stat",
        "trx_tvm",
        "trx_storage_stat",
        "trx_other",
        "final_storage_stat",
        "enqueue_new_messages",
        "combine_account_transactions",
        "create_shard_state",
        "create_block",
        "create_collated_data",
        "create_block_candidate",
        "other",
      ]
      const labels = [
        "Preinit",
        "Queue cleanup",
        "Prelim storage stat",
        "Trx TVM",
        "Trx storage stat",
        "Trx other",
        "Final storage stat",
        "Enqueue new messages",
        "Combine account transactions",
        "Create shard state",
        "Create block",
        "Create collated data",
        "Create block candidate",
        "Other work",
      ]
      const stats = fields.map(field => `BLOCK_collate_work_time_${time}_${field}`)
      if (time === "real") {
        stats.push("BLOCK_collate_time_wait_externals", "BLOCK_collate_time_other_wait")
        labels.push("Wait externals", "Other wait")
      }
      const chain = workchain === -1 ? "master" : "shards"
      charts.push(
        stackedChart(`Collate time (${chain}, ${time})`, workchain, stats, labels, "seconds"),
      )

      if (time === "real") {
        for (const source of sources) {
          charts.push(
            stackedChart(
              `Collate time (${chain}, ${time}) by ${source}`,
              workchain,
              stats.map(stat => `${stat} src=${source}`),
              labels,
              "seconds",
            ),
          )
        }
      }
    }
  }

  for (const workchain of [-1, 0]) {
    for (const time of ["real", "cpu"] as const) {
      const fields = [
        "unpack_block_candidate",
        "process_mc_state",
        "trx_tvm",
        "trx_storage_stat",
        "trx_other",
        "check_transactions_other",
        "unpack_state",
        "validate_block_tlb",
        "unpack_block_data",
        "precheck_account_updates",
        "precheck_account_transactions",
        "precheck_msg_queue",
        "unpack_dispatch_queue",
        "check_in_msg_descr",
        "check_out_msg_descr",
        "check_dispatch_queue",
        "check_processed_upto",
        "check_in_queue",
        "check_new_state",
        "other",
      ]
      const labels = [
        "Unpack block candidate",
        "Process mc state",
        "Trx TVM",
        "Trx storage stat",
        "Trx other",
        "Check transactions other",
        "Unpack state",
        "Validate block TLB",
        "Unpack block data",
        "Precheck account updates",
        "Precheck account transactions",
        "Precheck msg queue",
        "Unpack dispatch queue",
        "Check in msg descr",
        "Check out msg descr",
        "Check dispatch queue",
        "Check processed upto",
        "Check in queue",
        "Check new state",
        "Other work",
      ]
      const stats = fields.map(field => `BLOCK_validate_work_time_${time}_${field}`)
      if (time === "real") {
        stats.push("BLOCK_validate_time_other_wait")
        labels.push("Other wait")
      }
      const chain = workchain === -1 ? "master" : "shards"
      charts.push(
        stackedChart(`Validate time (${chain}, ${time})`, workchain, stats, labels, "seconds"),
      )
    }
  }

  charts.push(
    stackedChart(
      "Collate storage stat cache (count)",
      0,
      [
        "BLOCK_collate_storage_stat_cache_small_cnt",
        "BLOCK_collate_storage_stat_cache_hit_cnt",
        "BLOCK_collate_storage_stat_cache_miss_cnt",
      ],
      ["Small", "Hit", "Miss"],
      "count",
    ),
    stackedChart(
      "Collate storage stat cache (cells)",
      0,
      [
        "BLOCK_collate_storage_stat_cache_small_cells",
        "BLOCK_collate_storage_stat_cache_hit_cells",
        "BLOCK_collate_storage_stat_cache_miss_cells",
      ],
      ["Small", "Hit", "Miss"],
      "cells",
    ),
  )

  return charts
}

function singleChart(
  title: string,
  stat: string,
  minmax = false,
  mode: ValueMode = "avg",
  workchain?: number,
  unit: ChartUnit = "count",
): ChartDefinition {
  const metrics: MetricDefinition[] = []

  if (workchain === undefined || workchain === -1) {
    metrics.push({label: minmax ? "MC avg" : "Masterchain", mode, stat, workchain: -1})
    if (minmax) {
      metrics.push({label: "MC min", mode: "min", stat, workchain: -1})
      metrics.push({label: "MC max", mode: "max", stat, workchain: -1})
    }
  }
  if (workchain === undefined || workchain === 0) {
    metrics.push({label: minmax ? "WC avg" : "Workchain", mode, stat, workchain: 0})
    if (minmax) {
      metrics.push({label: "WC min", mode: "min", stat, workchain: 0})
      metrics.push({label: "WC max", mode: "max", stat, workchain: 0})
    }
  }

  return {kind: "lines", metrics, title, unit}
}

function lineChart(
  title: string,
  workchain: number,
  stats: readonly string[],
  labels: readonly string[],
  unit: ChartUnit,
): ChartDefinition {
  return {
    kind: "lines",
    metrics: stats.map((stat, index) => ({
      label: labels[index] ?? stat,
      mode: "avg",
      stat,
      workchain,
    })),
    title,
    unit,
  }
}

function stackedChart(
  title: string,
  workchain: number,
  stats: readonly string[],
  labels: readonly string[],
  unit: ChartUnit,
): ChartDefinition {
  return {...lineChart(title, workchain, stats, labels, unit), kind: "stacked"}
}

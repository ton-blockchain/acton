import type {TpsPoint, TpsView} from "../types"
import {useTheme} from "@acton/ui"
import {BarChart} from "echarts/charts"
import {
  AriaComponent,
  DataZoomComponent,
  GridComponent,
  MarkLineComponent,
  TooltipComponent,
} from "echarts/components"
import {init, use, type EChartsCoreOption, type EChartsType} from "echarts/core"
import {CanvasRenderer} from "echarts/renderers"
import {useEffect, useRef, useState} from "react"
import styles from "./TpsSection.module.css"

interface TpsSectionProps {
  readonly series: TpsView | undefined
}

type ChartMetric = "tps" | "block_time"

const DEFAULT_VISIBLE_SECONDS = 10 * 60

use([
  AriaComponent,
  BarChart,
  CanvasRenderer,
  DataZoomComponent,
  GridComponent,
  MarkLineComponent,
  TooltipComponent,
])

/** Presents related throughput and block-production timing from one indexed window */
function PerformanceSections({series}: TpsSectionProps) {
  return (
    <>
      <TpsSection series={series} />
      <BlockTimeSection series={series} />
    </>
  )
}

function TpsSection({series}: TpsSectionProps) {
  const points = series?.points ?? []
  const current = points.at(-1)
  const peak = points.reduce((maximum, point) => Math.max(maximum, point.tps), 0)
  const [visibleRange, setVisibleRange] = useState<ZoomRange>()
  const average = averageTps(points, series?.bucket_seconds, visibleRange)
  const queueSize = series?.queue_size

  return (
    <section id="throughput" className={styles.section} aria-labelledby="throughput-title">
      <div className={styles.heading}>
        <h2 id="throughput-title">Transaction throughput</h2>
      </div>
      <div className={styles.panel}>
        <div className={`${styles.summary} ${styles.summaryFour}`}>
          <SummaryMetric label="Current" value={current ? `${formatTps(current.tps)} TPS` : "—"} />
          <SummaryMetric
            label="Range average"
            value={average === undefined ? "—" : `${formatTps(average)} TPS`}
          />
          <SummaryMetric label="Peak" value={points.length > 0 ? `${formatTps(peak)} TPS` : "—"} />
          <SummaryMetric
            label="Queue"
            value={queueSize === undefined || queueSize === null ? "—" : queueSize.toLocaleString()}
          />
        </div>

        {series?.status === "unavailable" ? (
          <ChartState>TPS indexing is available on the network collector</ChartState>
        ) : points.length === 0 ? (
          <ChartState>Indexing recent blocks</ChartState>
        ) : (
          <MetricsChart metric="tps" points={points} onRangeChange={setVisibleRange} />
        )}
      </div>
    </section>
  )
}

function BlockTimeSection({series}: TpsSectionProps) {
  const points = series?.points ?? []
  const current = points.at(-1)
  const bucketDurationMs = (series?.bucket_seconds ?? 5) * 1000
  const observed = points.flatMap(point => {
    if (point.masterchain_blocks === 0) return [bucketDurationMs]
    return point.block_time_ms === null ? [] : [point.block_time_ms]
  })
  const p95 = percentile(observed, 0.95)
  const maximum = observed.length > 0 ? Math.max(...observed) : undefined
  const target = series?.block_time_target_ms

  return (
    <section id="block-time" className={styles.section} aria-labelledby="block-time-title">
      <div className={styles.heading}>
        <h2 id="block-time-title">Masterchain block time</h2>
      </div>
      <div className={styles.panel}>
        <div className={`${styles.summary} ${styles.summaryFour}`}>
          <SummaryMetric
            label="Target"
            value={target === undefined ? "—" : formatDuration(target)}
          />
          <SummaryMetric
            label="Current"
            value={
              current?.masterchain_blocks === 0
                ? "No blocks"
                : current?.block_time_ms === null || current?.block_time_ms === undefined
                  ? "—"
                  : formatDuration(current.block_time_ms)
            }
          />
          <SummaryMetric label="P95" value={p95 === undefined ? "—" : formatDuration(p95)} />
          <SummaryMetric
            label="Maximum"
            value={maximum === undefined ? "—" : formatDuration(maximum)}
          />
        </div>

        {series?.status === "unavailable" ? (
          <ChartState>Performance indexing is available on the network collector</ChartState>
        ) : points.length === 0 ? (
          <ChartState>Indexing recent blocks</ChartState>
        ) : (
          <MetricsChart
            bucketDurationMs={bucketDurationMs}
            metric="block_time"
            points={points}
            targetBlockTimeMs={target}
          />
        )}
      </div>
    </section>
  )
}

function SummaryMetric({label, value}: {readonly label: string; readonly value: string}) {
  return (
    <div className={styles.metric}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  )
}

function ChartState({children}: {readonly children: string}) {
  return (
    <div className={styles.chartState}>
      <span className={styles.pulse} aria-hidden="true" />
      <span>{children}</span>
    </div>
  )
}

function MetricsChart({
  bucketDurationMs,
  metric,
  onRangeChange,
  points,
  targetBlockTimeMs,
}: {
  readonly bucketDurationMs?: number
  readonly metric: ChartMetric
  readonly onRangeChange?: (range: ZoomRange) => void
  readonly points: readonly TpsPoint[]
  readonly targetBlockTimeMs?: number
}) {
  const elementRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<EChartsType | undefined>(undefined)
  const extentRef = useRef({from: 0, to: 0})
  const renderKeyRef = useRef("")
  const zoomRef = useRef<ZoomRange | undefined>(undefined)
  const {theme} = useTheme()

  useEffect(() => {
    const element = elementRef.current
    if (!element) return

    const chart = init(element, undefined, {renderer: "canvas"})
    chartRef.current = chart
    const resizeObserver = new ResizeObserver(() => chart.resize())
    resizeObserver.observe(element)

    chart.on("datazoom", rawEvent => {
      const event = rawEvent as DataZoomEvent
      const payload = Array.isArray(event.batch) ? event.batch[0] : event
      const start = Number(payload.start ?? 0)
      const end = Number(payload.end ?? 100)
      const extent = extentRef.current
      const duration = extent.to - extent.from

      const range = {
        automatic: false,
        followsLatest: end >= 99.5,
        from: extent.from + (duration * start) / 100,
        to: extent.from + (duration * end) / 100,
      }
      zoomRef.current = range
      onRangeChange?.(range)
    })

    return () => {
      resizeObserver.disconnect()
      chart.dispose()
      chartRef.current = undefined
    }
  }, [onRangeChange])

  useEffect(() => {
    const chart = chartRef.current
    const element = elementRef.current
    const firstPoint = points[0]
    const lastPoint = points.at(-1)
    if (!chart || !element || !firstPoint || !lastPoint) return

    const renderKey = `${metric}:${theme}:${targetBlockTimeMs ?? "none"}:${firstPoint.timestamp}:${lastPoint.timestamp}:${points.length}`
    if (renderKey === renderKeyRef.current) return
    renderKeyRef.current = renderKey

    const dataFrom = firstPoint.timestamp * 1000
    const to = lastPoint.timestamp * 1000
    const from = Math.min(dataFrom, to - DEFAULT_VISIBLE_SECONDS * 1000)
    extentRef.current = {from, to}
    const zoom = resolveZoomRange(zoomRef.current, from, to)
    zoomRef.current = zoom
    onRangeChange?.(zoom)
    const computed = getComputedStyle(element)
    const color = (name: string) => computed.getPropertyValue(name).trim()
    const isBlockTime = metric === "block_time"
    const barColor = color(isBlockTime ? "--block-time-bar-color" : "--tps-bar-color")
    const activeColor = color(
      isBlockTime ? "--block-time-bar-active-color" : "--tps-bar-active-color",
    )
    const referenceColor = color(
      isBlockTime ? "--block-time-reference-color" : "--tps-reference-color",
    )
    const delayedColor = color("--block-time-delayed-color")
    const stalledColor = color("--block-time-stalled-color")
    const chartDescription = isBlockTime
      ? "Mean masterchain block interval in five-second windows"
      : "Transaction throughput over time"

    chart.setOption({
      animation: false,
      aria: {
        enabled: true,
        description: chartDescription,
      },
      dataZoom: [
        {
          endValue: zoom.to,
          filterMode: "filter",
          moveOnMouseMove: true,
          moveOnMouseWheel: "shift",
          startValue: zoom.from,
          throttle: 16,
          type: "inside",
          zoomOnMouseWheel: "ctrl",
        },
        {
          backgroundColor: "transparent",
          borderColor: color("--acton-color-border"),
          bottom: 2,
          brushSelect: false,
          dataBackground: {
            areaStyle: {color: barColor},
            lineStyle: {color: activeColor},
          },
          endValue: zoom.to,
          fillerColor: color("--tps-selection-color"),
          handleSize: "80%",
          handleStyle: {
            borderColor: activeColor,
            color: color("--acton-color-surface-raised"),
          },
          height: 24,
          moveHandleStyle: {color: activeColor},
          selectedDataBackground: {
            areaStyle: {color: barColor},
            lineStyle: {color: activeColor},
          },
          showDetail: false,
          startValue: zoom.from,
          textStyle: {color: color("--acton-color-text-subtle"), fontSize: 10},
          type: "slider",
        },
      ],
      grid: {bottom: 48, containLabel: false, left: 50, right: 0, top: 10},
      series: [
        {
          barCategoryGap: "8%",
          barMaxWidth: 24,
          data: points.map(point => {
            const value = isBlockTime
              ? point.masterchain_blocks === 0
                ? (bucketDurationMs ?? null)
                : point.block_time_ms
              : point.tps
            const pointColor = isBlockTime
              ? point.masterchain_blocks === 0
                ? stalledColor
                : point.block_time_ms !== null &&
                    targetBlockTimeMs !== undefined &&
                    point.block_time_ms > targetBlockTimeMs
                  ? delayedColor
                  : barColor
              : barColor
            return {
              itemStyle: {color: pointColor},
              value: [point.timestamp * 1000, value, point.transactions, point.masterchain_blocks],
            }
          }),
          itemStyle: {color: barColor},
          markLine: {
            animation: false,
            data:
              isBlockTime && targetBlockTimeMs !== undefined
                ? [{yAxis: targetBlockTimeMs}]
                : [{type: "average"}],
            label: {
              color: referenceColor,
              formatter: ({value}: {value?: number | string}) => formatTps(Number(value ?? 0)),
              fontSize: 11,
              fontWeight: 600,
              position: "start",
              show: !isBlockTime,
            },
            lineStyle: {color: referenceColor, type: "dashed"},
            silent: true,
            symbol: ["none", "none"],
          },
          name: isBlockTime ? "Block time" : "Throughput",
          type: "bar",
        },
      ],
      tooltip: {
        appendToBody: false,
        axisPointer: {type: "shadow"},
        backgroundColor: "transparent",
        borderWidth: 0,
        confine: true,
        extraCssText: "box-shadow: none",
        formatter: (params: unknown) => formatChartTooltip(params, metric),
        padding: 0,
        trigger: "axis",
      },
      xAxis: {
        axisLabel: {
          color: color("--acton-color-text-subtle"),
          fontSize: 11,
          formatter: (value: number) => formatTime(value / 1000),
          hideOverlap: true,
        },
        axisLine: {show: false},
        axisTick: {show: false},
        max: to,
        min: from,
        minInterval: 5000,
        splitLine: {
          lineStyle: {color: color("--acton-color-border"), type: "dashed"},
          show: true,
        },
        type: "time",
      },
      yAxis: {
        axisLabel: {
          color: color("--acton-color-text-subtle"),
          fontSize: 11,
          formatter: (value: number) =>
            isBlockTime ? formatDuration(value, true) : formatAxis(value),
        },
        axisLine: {show: false},
        axisTick: {show: false},
        min: 0,
        splitLine: {show: false},
        splitNumber: 2,
        type: "value",
      },
    } satisfies EChartsCoreOption)
  }, [bucketDurationMs, metric, onRangeChange, points, targetBlockTimeMs, theme])

  return (
    <div className={`${styles.chart} ${metric === "block_time" ? styles.blockTimeChart : ""}`}>
      <div
        ref={elementRef}
        className={styles.chartCanvas}
        role="img"
        aria-label={
          metric === "block_time"
            ? "Mean masterchain block interval in five-second windows"
            : "Transaction throughput over time"
        }
      />
    </div>
  )
}

interface ZoomRange {
  readonly automatic: boolean
  readonly followsLatest: boolean
  readonly from: number
  readonly to: number
}

interface DataZoomEvent {
  readonly batch?: readonly DataZoomEvent[]
  readonly end?: number
  readonly start?: number
}

function resolveZoomRange(previous: ZoomRange | undefined, from: number, to: number): ZoomRange {
  if (!previous || previous.automatic) {
    return {
      automatic: true,
      followsLatest: true,
      from: Math.max(from, to - DEFAULT_VISIBLE_SECONDS * 1000),
      to,
    }
  }

  const duration = Math.max(5000, previous.to - previous.from)
  if (previous.followsLatest) {
    return {automatic: false, followsLatest: true, from: Math.max(from, to - duration), to}
  }

  const nextFrom = Math.max(from, previous.from)
  const nextTo = Math.min(to, Math.max(nextFrom + 5000, previous.to))
  return {automatic: false, followsLatest: false, from: nextFrom, to: nextTo}
}

function averageTps(
  points: readonly TpsPoint[],
  bucketSeconds: number | undefined,
  range: ZoomRange | undefined,
): number | undefined {
  if (points.length === 0 || bucketSeconds === undefined || bucketSeconds <= 0) return undefined

  const latestTimestamp = points.at(-1)?.timestamp ?? 0
  const selectedFrom = range?.from ?? (latestTimestamp - DEFAULT_VISIBLE_SECONDS) * 1000
  const selectedTo = range?.to ?? latestTimestamp * 1000
  const visiblePoints = points.filter(point => {
    const timestamp = point.timestamp * 1000
    return timestamp >= selectedFrom && timestamp <= selectedTo
  })
  const first = visiblePoints[0]
  const last = visiblePoints.at(-1)
  if (!first || !last) return undefined

  // Include empty time between buckets so gaps cannot inflate the selected-range average
  const coveredSeconds = Math.max(bucketSeconds, last.timestamp - first.timestamp + bucketSeconds)
  const transactions = visiblePoints.reduce((total, point) => total + point.transactions, 0)

  return transactions / coveredSeconds
}

function formatChartTooltip(params: unknown, metric: ChartMetric) {
  const item = Array.isArray(params) ? params[0] : params
  if (!item || typeof item !== "object" || !("value" in item) || !Array.isArray(item.value)) {
    return ""
  }

  const timestamp = Number(item.value[0]) / 1000
  const value = item.value[1] === null ? null : Number(item.value[1])
  const transactions = Number(item.value[2])
  const masterchainBlocks = Number(item.value[3])

  if (metric === "block_time") {
    return `<div class="${styles.tooltip}">
      <strong class="${styles.tooltipTime}">${formatTooltipTime(timestamp)}</strong>
      <div class="${styles.tooltipValue}">
        <span class="${styles.tooltipDot}"></span>
        <span>Block time:</span>
        <strong>${masterchainBlocks === 0 ? "No blocks" : value === null ? "—" : formatDuration(value)}</strong>
      </div>
      <div class="${styles.tooltipValue}">
        <span>Masterchain blocks:</span>
        <strong>${masterchainBlocks.toLocaleString()}</strong>
      </div>
      <div class="${styles.tooltipValue}">
        <span>Transactions:</span>
        <strong>${transactions.toLocaleString()}</strong>
      </div>
    </div>`
  }

  return `<div class="${styles.tooltip}">
    <strong class="${styles.tooltipTime}">${formatTooltipTime(timestamp)}</strong>
    <div class="${styles.tooltipValue}">
      <span class="${styles.tooltipDot}"></span>
      <span>Throughput:</span>
      <strong>${formatTps(value ?? 0)} TPS</strong>
    </div>
  </div>`
}

function formatTps(value: number) {
  return value.toLocaleString(undefined, {maximumFractionDigits: value < 10 ? 2 : 1})
}

function formatAxis(value: number) {
  return value.toLocaleString(undefined, {maximumFractionDigits: value < 10 ? 1 : 0})
}

function formatDuration(value: number, compact = false) {
  if (value < 1000) return `${Math.round(value)} ms`
  const seconds = value / 1000
  return `${seconds.toLocaleString(undefined, {maximumFractionDigits: compact ? 1 : 2})} s`
}

function percentile(values: readonly number[], fraction: number) {
  if (values.length === 0) return undefined
  const sorted = [...values].sort((left, right) => left - right)
  return sorted[Math.ceil(fraction * sorted.length) - 1]
}

function formatTime(timestamp: number) {
  return new Date(timestamp * 1000).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
}

function formatTooltipTime(timestamp: number) {
  return new Date(timestamp * 1000).toLocaleString([], {
    month: "long",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
}

export default PerformanceSections

import type {HTMLAttributes, ReactNode} from "react"

import {DAY_SECONDS} from "../../lib/time"
import {CopyInlineAction} from "../InlineActions/InlineActions"
import {Tooltip, type TooltipPlacement} from "../Tooltip"

import styles from "./Duration.module.css"

export type DurationUnit = "nanoseconds" | "milliseconds" | "seconds"
export type DurationDisplay =
  | "compact"
  | "elapsed"
  | "human"
  | "latency"
  | "parts"
  | "precise"
  | "readable"
  | "runtime"
  | "startup"

export type DurationSign = "auto" | "always" | "never"

export interface DurationFormatOptions {
  readonly display?: DurationDisplay
  readonly locale?: string
  readonly maxParts?: number
  readonly sign?: DurationSign
  readonly unit?: DurationUnit
}

export interface RecurringPeriodFormatOptions {
  readonly unit?: DurationUnit
}

export interface DurationProps
  extends Omit<HTMLAttributes<HTMLSpanElement>, "children">,
    DurationFormatOptions {
  readonly fallback?: ReactNode
  readonly tooltip?: boolean
  readonly tooltipPlacement?: TooltipPlacement
  readonly value: number | null | undefined
}

const SECOND_MS = 1000
const MINUTE_SECONDS = 60
const HOUR_SECONDS = 3600
const WEEK_SECONDS = 604_800
const MONTH_SECONDS = 2_592_000
const YEAR_SECONDS = 31_536_000

const DURATION_PARTS = [
  {seconds: YEAR_SECONDS, compact: "y", name: "year"},
  {seconds: MONTH_SECONDS, compact: "mo", name: "month"},
  {seconds: WEEK_SECONDS, compact: "w", name: "week"},
  {seconds: DAY_SECONDS, compact: "d", name: "day"},
  {seconds: HOUR_SECONDS, compact: "h", name: "hour"},
  {seconds: MINUTE_SECONDS, compact: "min", name: "minute"},
  {seconds: 1, compact: "s", name: "second"},
] as const

export function Duration({
  value,
  fallback = "—",
  display,
  locale,
  maxParts,
  sign,
  unit,
  tooltip = true,
  tooltipPlacement = "top",
  title,
  className,
  tabIndex,
  ...props
}: DurationProps) {
  if (!isDurationValue(value)) return fallback

  const duration = (
    <span
      data-visual-dynamic="duration"
      data-visual-placeholder="<duration>"
      {...props}
      className={
        [className, tooltip ? styles.trigger : undefined].filter(Boolean).join(" ") || undefined
      }
      tabIndex={tooltip ? (tabIndex ?? 0) : tabIndex}
    >
      {formatDuration(value, {display, locale, maxParts, sign, unit})}
    </span>
  )

  return tooltip ? (
    <Tooltip
      content={<DurationTooltip heading={title} unit={unit} value={value} />}
      placement={tooltipPlacement}
      width="wide"
    >
      {duration}
    </Tooltip>
  ) : (
    duration
  )
}

function DurationTooltip({
  heading,
  unit,
  value,
}: {
  readonly heading?: string
  readonly unit?: DurationUnit
  readonly value: number
}) {
  const rawValue = String(value)
  const resolvedUnit = unit ?? "seconds"

  return (
    <span className={styles.tooltip}>
      {heading ? <strong>{heading}</strong> : undefined}
      <span className={styles.tooltipRow}>
        <span>Raw value</span>
        <span className={styles.tooltipCopyValue}>
          <code>{rawValue}</code>
          <CopyInlineAction
            copiedLabel="Raw value copied"
            label="Copy raw value"
            size="compact"
            value={rawValue}
          />
        </span>
      </span>
      <span className={styles.tooltipRow}>
        <span>Unit</span>
        <span>{resolvedUnit}</span>
      </span>
    </span>
  )
}

export function formatDuration(
  value: number | null | undefined,
  options: DurationFormatOptions = {},
): string {
  if (!isDurationValue(value)) return "—"

  const display = options.display ?? "compact"
  if (display === "latency")
    return formatLatency(toNanoseconds(value, options.unit), options.locale)
  if (display === "startup") return formatStartup(toMilliseconds(value, options.unit))
  if (display === "runtime") return formatRuntime(toMilliseconds(value, options.unit))
  if (display === "precise") return formatPrecise(toMilliseconds(value, options.unit))

  const seconds = toSeconds(value, options.unit)
  if (display === "elapsed") return formatElapsed(seconds, options.sign)
  if (display === "human") return formatHuman(seconds)
  if (display === "parts") {
    return formatParts(seconds, {
      compact: true,
      maxParts: options.maxParts,
      sign: options.sign,
    })
  }
  if (display === "readable") {
    return formatParts(seconds, {
      compact: false,
      maxParts: options.maxParts,
      sign: options.sign,
    })
  }
  return formatCompact(seconds, options.sign)
}

/**
 * Formats an exact schedule interval in the largest whole supported unit.
 *
 * The input uses seconds. Exact days, hours, and minutes use a singular or
 * plural unit. Other values stay in seconds so the function never hides
 * precision. Zero returns `0 seconds`. Invalid values return `—`.
 *
 * @example formatSchedulePeriod(7_200) // "2 hours"
 * @example formatSchedulePeriod(90) // "90 seconds"
 */
export function formatSchedulePeriod(seconds: number): string {
  if (!isDurationValue(seconds)) return "—"
  if (seconds === 0) return "0 seconds"
  if (seconds % DAY_SECONDS === 0) {
    const days = seconds / DAY_SECONDS
    return `${days} ${days === 1 ? "day" : "days"}`
  }
  if (seconds % HOUR_SECONDS === 0) {
    const hours = seconds / HOUR_SECONDS
    return `${hours} ${hours === 1 ? "hour" : "hours"}`
  }
  if (seconds % MINUTE_SECONDS === 0) {
    const minutes = seconds / MINUTE_SECONDS
    return `${minutes} ${minutes === 1 ? "minute" : "minutes"}`
  }
  return `${seconds.toLocaleString()} seconds`
}

/**
 * Formats a recurring request or refresh window.
 *
 * Numeric values use milliseconds by default. One hour and one day use
 * `per hour` and `per day`. Other values are rounded to the nearest minute
 * and use `every N minutes`, with a minimum of one minute.
 * Invalid values return `—`.
 *
 * @example formatRecurringPeriod(3_600_000) // "per hour"
 * @example formatRecurringPeriod(5_400_000) // "every 90 minutes"
 */
export function formatRecurringPeriod(
  value: number,
  options: RecurringPeriodFormatOptions = {},
): string {
  if (!isDurationValue(value)) return "—"
  const seconds = toSeconds(value, options.unit ?? "milliseconds")
  if (seconds === HOUR_SECONDS) return "per hour"
  if (seconds === DAY_SECONDS) return "per day"

  const minutes = Math.max(1, Math.round(seconds / MINUTE_SECONDS))
  return `every ${formatSchedulePeriod(minutes * MINUTE_SECONDS)}`
}

function isDurationValue(value: number | null | undefined): value is number {
  return value !== null && value !== undefined && Number.isFinite(value)
}

function toNanoseconds(value: number, unit: DurationUnit = "seconds"): number {
  if (unit === "nanoseconds") return value
  if (unit === "milliseconds") return value * 1_000_000
  return value * 1_000_000_000
}

function toMilliseconds(value: number, unit: DurationUnit = "seconds"): number {
  if (unit === "nanoseconds") return value / 1_000_000
  if (unit === "milliseconds") return value
  return value * SECOND_MS
}

function toSeconds(value: number, unit: DurationUnit = "seconds"): number {
  if (unit === "nanoseconds") return value / 1_000_000_000
  if (unit === "milliseconds") return value / SECOND_MS
  return value
}

function durationSign(value: number, sign: DurationSign = "auto"): string {
  if (sign === "never") return ""
  if (value < 0) return "-"
  return sign === "always" ? "+" : ""
}

function formatCompact(seconds: number, sign: DurationSign = "auto"): string {
  const prefix = durationSign(seconds, sign)
  const value = Math.abs(seconds)
  if (value < MINUTE_SECONDS) return `${prefix}${value}s`
  if (value < HOUR_SECONDS) return `${prefix}${Math.floor(value / MINUTE_SECONDS)}m`
  if (value < DAY_SECONDS) return `${prefix}${Math.floor(value / HOUR_SECONDS)}h`
  return `${prefix}${Math.floor(value / DAY_SECONDS)}d`
}

function formatElapsed(seconds: number, sign: DurationSign = "auto"): string {
  const prefix = durationSign(seconds, sign)
  const wholeSeconds = Math.floor(Math.abs(seconds))
  const minutes = Math.floor(wholeSeconds / MINUTE_SECONDS)
  const remainingSeconds = wholeSeconds % MINUTE_SECONDS
  return minutes > 0
    ? `${prefix}${minutes}m ${remainingSeconds.toString().padStart(2, "0")}s`
    : `${prefix}${remainingSeconds}s`
}

function formatHuman(seconds: number): string {
  if (seconds <= 0) return "Less than 1 second"
  if (seconds === 1) return "1 second"
  if (seconds < MINUTE_SECONDS) return `${seconds} seconds`

  const minutes = Math.floor(seconds / MINUTE_SECONDS)
  const remainingSeconds = seconds % MINUTE_SECONDS
  return remainingSeconds === 0 ? `${minutes} min` : `${minutes} min ${remainingSeconds} sec`
}

function formatStartup(milliseconds: number): string {
  if (milliseconds < SECOND_MS) return `${milliseconds} ms`
  const seconds = milliseconds / SECOND_MS
  return `${seconds < 10 ? seconds.toFixed(1) : seconds.toFixed(0)} s`
}

function formatRuntime(milliseconds: number): string {
  if (milliseconds < SECOND_MS) return `${milliseconds} ms`
  if (milliseconds < MINUTE_SECONDS * SECOND_MS) {
    return `${(milliseconds / SECOND_MS).toFixed(1)} s`
  }
  return `${Math.floor(milliseconds / (MINUTE_SECONDS * SECOND_MS))}m ${Math.round(
    (milliseconds % (MINUTE_SECONDS * SECOND_MS)) / SECOND_MS,
  )}s`
}

function formatPrecise(milliseconds: number): string {
  if (milliseconds < 1) return `${(milliseconds * 1000).toFixed(0)}µs`
  if (milliseconds < SECOND_MS) return `${milliseconds.toFixed(1)}ms`
  return `${(milliseconds / SECOND_MS).toFixed(2)}s`
}

function formatLatency(nanoseconds: number, locale: string | undefined): string {
  if (nanoseconds < 1000) return `${Math.round(nanoseconds)} ns`
  if (nanoseconds < 1_000_000) return `${formatLatencyValue(nanoseconds / 1000, locale)} µs`

  const milliseconds = nanoseconds / 1_000_000
  if (milliseconds < 10) return `${formatLatencyValue(milliseconds, locale)} ms`
  return `${Math.round(milliseconds)} ms`
}

function formatLatencyValue(value: number, locale: string | undefined): string {
  return value.toLocaleString(locale, {
    maximumFractionDigits: value < 10 ? 2 : value < 100 ? 1 : 0,
  })
}

function formatParts(
  seconds: number,
  options: {
    readonly compact: boolean
    readonly maxParts?: number
    readonly sign?: DurationSign
  },
): string {
  const prefix = durationSign(seconds, options.sign)
  let remainingSeconds = Math.abs(seconds)
  const parts: string[] = []

  for (const unit of DURATION_PARTS) {
    const value = Math.floor(remainingSeconds / unit.seconds)
    if (value === 0) continue

    parts.push(
      options.compact
        ? `${value}${unit.compact}`
        : `${value} ${value === 1 ? unit.name : `${unit.name}s`}`,
    )
    remainingSeconds %= unit.seconds
    if (options.maxParts !== undefined && parts.length === options.maxParts) break
  }

  const zeroValue = options.compact ? "0s" : "0 seconds"
  return `${prefix}${parts.length > 0 ? parts.join(" ") : zeroValue}`
}

import type {HTMLAttributes, ReactNode} from "react"

import {DAY_SECONDS} from "../../lib/time"
import {Tooltip, type TooltipPlacement} from "../Tooltip"
import {CopyInlineAction} from "../InlineActions/InlineActions"

import styles from "./DateTime.module.css"

export type DateTimeValue = Date | number | string
export type DateTimeUnit = "milliseconds" | "seconds"
export type DateTimeDisplay =
  | "date"
  | "date-long"
  | "date-numeric"
  | "date-day-month"
  | "date-time"
  | "date-time-day-month"
  | "date-time-day-month-short"
  | "date-time-numeric"
  | "date-time-numeric-seconds"
  | "date-time-seconds"
  | "time"
  | "time-seconds"
  | "compact"
  | "smart"
  | "month-short"
  | "month-long"

interface DateTimeFormatOptions {
  readonly display?: DateTimeDisplay
  readonly locale?: string
  readonly now?: DateTimeValue
  readonly timeZone?: string
  readonly unit?: DateTimeUnit
}

interface RelativeDateTimeFormatOptions extends Omit<DateTimeFormatOptions, "display"> {
  readonly mode?: "hybrid" | "relative"
}

export interface DateTimeLocalInputOptions {
  readonly timeZone?: string
  readonly unit?: DateTimeUnit
}

export interface DateTimeProps
  extends Omit<HTMLAttributes<HTMLTimeElement>, "children" | "dateTime">,
    DateTimeFormatOptions {
  readonly fallback?: ReactNode
  readonly tooltip?: boolean
  readonly tooltipPlacement?: TooltipPlacement
  readonly value: DateTimeValue | null | undefined
}

export interface RelativeTimeProps
  extends Omit<HTMLAttributes<HTMLTimeElement>, "children" | "dateTime">,
    RelativeDateTimeFormatOptions {
  readonly fallback?: ReactNode
  readonly tooltip?: boolean
  readonly tooltipPlacement?: TooltipPlacement
  readonly value: DateTimeValue | null | undefined
}

const DISPLAY_OPTIONS = {
  date: {dateStyle: "medium"},
  "date-long": {dateStyle: "long"},
  "date-time": {dateStyle: "medium", timeStyle: "short", hourCycle: "h23"},
  "date-time-seconds": {dateStyle: "medium", timeStyle: "medium", hourCycle: "h23"},
  time: {hour: "2-digit", minute: "2-digit", hourCycle: "h23"},
  "time-seconds": {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
  },
  "month-short": {month: "short", year: "2-digit"},
  "month-long": {month: "long", year: "numeric"},
} as const satisfies Partial<Record<DateTimeDisplay, Intl.DateTimeFormatOptions>>

const formatterCache = new Map<string, Intl.DateTimeFormat>()

export function DateTime({
  value,
  unit,
  display,
  locale,
  now,
  timeZone,
  fallback = "—",
  tooltip = true,
  tooltipPlacement = "top",
  title,
  className,
  tabIndex,
  ...props
}: DateTimeProps) {
  const date = dateTimeValueToDate(value, unit)
  if (!date) return fallback

  const resolvedDisplay = display ?? "date-time"
  const time = (
    <time
      data-visual-dynamic="time"
      data-visual-placeholder="<time>"
      {...props}
      className={
        [className, tooltip ? styles.trigger : undefined].filter(Boolean).join(" ") || undefined
      }
      dateTime={date.toISOString()}
      tabIndex={tooltip ? (tabIndex ?? 0) : tabIndex}
    >
      {formatDateTime(date, {display: resolvedDisplay, locale, now, timeZone, unit})}
    </time>
  )

  return tooltip ? (
    <Tooltip
      content={<DateTimeTooltip date={date} heading={title} locale={locale} timeZone={timeZone} />}
      placement={tooltipPlacement}
      width="wide"
    >
      {time}
    </Tooltip>
  ) : (
    time
  )
}

export function RelativeTime({
  value,
  unit,
  locale,
  now,
  timeZone,
  mode,
  fallback = "—",
  tooltip = true,
  tooltipPlacement = "top",
  title,
  className,
  tabIndex,
  ...props
}: RelativeTimeProps) {
  const date = dateTimeValueToDate(value, unit)
  if (!date) return fallback

  const time = (
    <time
      data-visual-dynamic="time"
      data-visual-placeholder="<time>"
      {...props}
      className={
        [className, tooltip ? styles.trigger : undefined].filter(Boolean).join(" ") || undefined
      }
      dateTime={date.toISOString()}
      tabIndex={tooltip ? (tabIndex ?? 0) : tabIndex}
    >
      {formatRelativeDateTime(date, {locale, mode, now, timeZone, unit})}
    </time>
  )

  return tooltip ? (
    <Tooltip
      content={<DateTimeTooltip date={date} heading={title} locale={locale} timeZone={timeZone} />}
      placement={tooltipPlacement}
      width="wide"
    >
      {time}
    </Tooltip>
  ) : (
    time
  )
}

export function formatDateTime(
  value: DateTimeValue | null | undefined,
  options: DateTimeFormatOptions = {},
): string {
  const date = dateTimeValueToDate(value, options.unit)
  if (!date) return "—"

  const display = options.display ?? "date-time"
  if (display === "date-numeric") {
    return formatNumericDateTime(date, options, false, false)
  }
  if (display === "date-day-month") {
    return formatDayMonthDateTime(date, options, {
      includeTime: false,
      padDay: true,
      year: "always",
    })
  }
  if (display === "date-time-day-month") {
    return formatDayMonthDateTime(date, options, {
      includeTime: true,
      padDay: true,
      year: "always",
    })
  }
  if (display === "date-time-day-month-short") {
    return formatDayMonthDateTime(date, options, {
      includeTime: true,
      padDay: true,
      year: "never",
    })
  }
  if (display === "date-time-numeric") {
    return formatNumericDateTime(date, options, true, false)
  }
  if (display === "date-time-numeric-seconds") {
    return formatNumericDateTime(date, options, true, true)
  }
  if (display === "compact") {
    return formatCompactDateTime(date, options)
  }
  if (display === "smart") {
    return formatSmartDateTime(date, options)
  }

  return getFormatter(options.locale, {
    ...DISPLAY_OPTIONS[display],
    timeZone: options.timeZone,
  }).format(date)
}

export function formatRelativeDateTime(
  value: DateTimeValue | null | undefined,
  options: RelativeDateTimeFormatOptions = {},
): string {
  const date = dateTimeValueToDate(value, options.unit)
  if (!date) return "—"

  const now = resolveNow(options.now, options.unit)
  const differenceSeconds = Math.trunc((date.getTime() - now.getTime()) / 1000)
  if (differenceSeconds === 0) return "right now"

  if ((options.mode ?? "relative") === "hybrid" && differenceSeconds <= -DAY_SECONDS) {
    return formatDateTime(date, {...options, display: "compact", now})
  }

  const future = differenceSeconds > 0
  const difference = Math.abs(differenceSeconds)
  const {amount, unit} = relativePart(difference, future)
  return future ? `in ${amount}${unit}` : `${amount}${unit} ago`
}

/**
 * Describes how long remains before a UNIX timestamp.
 *
 * Both arguments use seconds. The result uses weeks for intervals of at least
 * seven days, days for intervals of at least one day, and hours for intervals
 * of at least one hour. Shorter or elapsed intervals return `soon`.
 *
 * @example formatTimeUntil(7_200, 0) // "in 2 hours"
 */
export function formatTimeUntil(timestampSeconds: number, nowSeconds: number): string {
  const remaining = Math.max(0, timestampSeconds - nowSeconds)
  if (remaining >= 7 * DAY_SECONDS) {
    const days = Math.ceil(remaining / DAY_SECONDS)
    const weeks = Math.max(1, Math.round(days / 7))
    return `in ${weeks} ${weeks === 1 ? "week" : "weeks"}`
  }
  if (remaining >= DAY_SECONDS) {
    const days = Math.ceil(remaining / DAY_SECONDS)
    return `in ${days} ${days === 1 ? "day" : "days"}`
  }

  if (remaining >= 3600) {
    const hours = Math.ceil(remaining / 3600)
    return `in ${hours} ${hours === 1 ? "hour" : "hours"}`
  }
  return "soon"
}

/**
 * Formats a timestamp for an HTML `input[type="datetime-local"]`.
 *
 * The result always uses `YYYY-MM-DDTHH:mm:ss` and has no UTC suffix because
 * datetime-local values represent wall-clock time. Set `unit` for numeric
 * timestamps and `timeZone` when the wall-clock zone must be explicit.
 * Invalid or missing values return an empty string.
 *
 * @example formatDateTimeLocalInput(1_785_925_815, {unit: "seconds", timeZone: "UTC"})
 * // "2026-08-05T10:30:15"
 */
export function formatDateTimeLocalInput(
  value: DateTimeValue | null | undefined,
  options: DateTimeLocalInputOptions = {},
): string {
  const date = dateTimeValueToDate(value, options.unit)
  if (!date) return ""

  const parts = getFormatter("en-GB", {
    day: "2-digit",
    hour: "2-digit",
    hourCycle: "h23",
    minute: "2-digit",
    month: "2-digit",
    second: "2-digit",
    timeZone: options.timeZone,
    year: "numeric",
  })
    .formatToParts(date)
    .reduce<Record<string, string>>((result, part) => {
      if (part.type !== "literal") result[part.type] = part.value
      return result
    }, {})

  return `${parts.year}-${parts.month}-${parts.day}T${parts.hour}:${parts.minute}:${parts.second}`
}

export function dateTimeValueToDate(
  value: DateTimeValue | null | undefined,
  unit: DateTimeUnit = "milliseconds",
): Date | undefined {
  if (value === null || value === undefined) return undefined

  const date =
    value instanceof Date
      ? new Date(value.getTime())
      : new Date(typeof value === "number" && unit === "seconds" ? value * 1000 : value)

  return Number.isNaN(date.getTime()) ? undefined : date
}

function relativePart(seconds: number, future: boolean): {amount: number; unit: string} {
  if (seconds < 60) return {amount: seconds, unit: "s"}
  if (seconds < 3600) {
    return {amount: future ? Math.ceil(seconds / 60) : Math.floor(seconds / 60), unit: "m"}
  }
  if (seconds < DAY_SECONDS) {
    return {amount: future ? Math.ceil(seconds / 3600) : Math.floor(seconds / 3600), unit: "h"}
  }
  if (seconds < 604_800) {
    return {
      amount: future ? Math.ceil(seconds / DAY_SECONDS) : Math.floor(seconds / DAY_SECONDS),
      unit: "d",
    }
  }
  if (seconds < 2_629_800) {
    return {
      amount: future ? Math.ceil(seconds / 604_800) : Math.floor(seconds / 604_800),
      unit: "w",
    }
  }
  if (seconds < 31_557_600) {
    return {
      amount: future ? Math.ceil(seconds / 2_629_800) : Math.floor(seconds / 2_629_800),
      unit: "mo",
    }
  }
  return {
    amount: future ? Math.ceil(seconds / 31_557_600) : Math.floor(seconds / 31_557_600),
    unit: "y",
  }
}

function formatCompactDateTime(date: Date, options: DateTimeFormatOptions): string {
  return formatDayMonthDateTime(date, options, {
    includeTime: true,
    padDay: false,
    year: "auto",
  })
}

function formatDayMonthDateTime(
  date: Date,
  options: DateTimeFormatOptions,
  format: {
    readonly includeTime: boolean
    readonly padDay: boolean
    readonly year: "always" | "auto" | "never"
  },
): string {
  const now = resolveNow(options.now, options.unit)
  const month = getFormatter(options.locale, {
    month: "short",
    timeZone: options.timeZone,
  }).format(date)
  const day = getFormatter(options.locale, {
    day: format.padDay ? "2-digit" : "numeric",
    timeZone: options.timeZone,
  }).format(date)
  const dateYear = getDatePart(date, "year", options.timeZone)
  const nowYear = getDatePart(now, "year", options.timeZone)
  const year =
    format.year === "always" || (format.year === "auto" && dateYear !== nowYear)
      ? ` ${dateYear}`
      : ""
  if (!format.includeTime) return `${day} ${month}${year}`

  const time = getFormatter(options.locale, {
    ...DISPLAY_OPTIONS.time,
    timeZone: options.timeZone,
  }).format(date)
  return `${day} ${month}${year}, ${time}`
}

function formatSmartDateTime(date: Date, options: DateTimeFormatOptions): string {
  const now = resolveNow(options.now, options.unit)
  const sameDay =
    getDatePart(date, "year", options.timeZone) === getDatePart(now, "year", options.timeZone) &&
    getDatePart(date, "month", options.timeZone) === getDatePart(now, "month", options.timeZone) &&
    getDatePart(date, "day", options.timeZone) === getDatePart(now, "day", options.timeZone)

  if (sameDay) {
    return formatDateTime(date, {...options, display: "time"})
  }

  return getFormatter(options.locale, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    timeZone: options.timeZone,
  }).format(date)
}

function formatNumericDateTime(
  date: Date,
  options: DateTimeFormatOptions,
  includeTime: boolean,
  includeSeconds: boolean,
): string {
  const parts = getFormatter("en-GB", {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    ...(includeTime
      ? {
          hour: "2-digit",
          minute: "2-digit",
          ...(includeSeconds ? {second: "2-digit"} : {}),
          hourCycle: "h23",
        }
      : {}),
    timeZone: options.timeZone,
  })
    .formatToParts(date)
    .reduce<Record<string, string>>((result, part) => {
      if (part.type !== "literal") result[part.type] = part.value
      return result
    }, {})

  const datePart = `${parts.day}.${parts.month}.${parts.year}`
  if (!includeTime) return datePart

  const timePart = includeSeconds
    ? `${parts.hour}:${parts.minute}:${parts.second}`
    : `${parts.hour}:${parts.minute}`
  return `${datePart}, ${timePart}`
}

function getDatePart(date: Date, part: "day" | "month" | "year", timeZone?: string): string {
  return getFormatter("en", {[part]: "numeric", timeZone}).format(date)
}

function resolveNow(now: DateTimeValue | undefined, unit: DateTimeUnit = "milliseconds"): Date {
  return now === undefined ? new Date() : (dateTimeValueToDate(now, unit) ?? new Date())
}

function getFormatter(locale: string | undefined, options: Intl.DateTimeFormatOptions) {
  const key = JSON.stringify([locale ?? null, options])
  const cached = formatterCache.get(key)
  if (cached) return cached

  const formatter = new Intl.DateTimeFormat(locale, options)
  formatterCache.set(key, formatter)
  return formatter
}

function DateTimeTooltip({
  date,
  heading,
  locale,
  timeZone,
}: {
  readonly date: Date
  readonly heading?: string
  readonly locale?: string
  readonly timeZone?: string
}) {
  const resolvedTimeZone =
    timeZone ?? new Intl.DateTimeFormat(locale).resolvedOptions().timeZone ?? "Local time"
  const offset = getFormatter(locale ?? "en", {
    timeZone: resolvedTimeZone === "Local time" ? undefined : resolvedTimeZone,
    timeZoneName: "longOffset",
  })
    .formatToParts(date)
    .find(part => part.type === "timeZoneName")?.value

  return (
    <span className={styles.tooltip}>
      {heading ? <strong>{heading}</strong> : undefined}
      <span className={styles.tooltipPrimary}>
        {formatDateTime(date, {
          display: "date-time-numeric-seconds",
          locale,
          timeZone,
        })}
      </span>
      <span className={styles.tooltipRow}>
        <span>Relative</span>
        <span>{formatRelativeDateTime(date)}</span>
      </span>
      <span className={styles.tooltipRow}>
        <span>Time zone</span>
        <span>{offset ? `${resolvedTimeZone} · ${offset}` : resolvedTimeZone}</span>
      </span>
      <span className={styles.tooltipRow}>
        <span>UNIX</span>
        <span className={styles.tooltipCopyValue}>
          <code>{Math.floor(date.getTime() / 1000)}</code>
          <CopyInlineAction
            copiedLabel="UNIX timestamp copied"
            label="Copy UNIX timestamp"
            size="compact"
            value={Math.floor(date.getTime() / 1000).toString()}
          />
        </span>
      </span>
      <span className={styles.tooltipRow}>
        <span>ISO</span>
        <span className={styles.tooltipCopyValue}>
          <code>{date.toISOString()}</code>
          <CopyInlineAction
            copiedLabel="ISO timestamp copied"
            label="Copy ISO timestamp"
            size="compact"
            value={date.toISOString()}
          />
        </span>
      </span>
    </span>
  )
}

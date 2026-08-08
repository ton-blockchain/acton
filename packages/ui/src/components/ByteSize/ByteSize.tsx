import type {HTMLAttributes, ReactNode} from "react"

import {formatNumberValue} from "../NumberValue"

const BYTE_BASE = 1024
const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"] as const

export interface ByteSizeFormatOptions {
  readonly fallback?: string
  readonly locale?: string
  readonly maximumFractionDigits?: number
}

export interface ByteSizeProps
  extends Omit<HTMLAttributes<HTMLDataElement>, "children" | "value">,
    Omit<ByteSizeFormatOptions, "fallback"> {
  readonly fallback?: ReactNode
  readonly value: number | null | undefined
}

/** Formats a byte count with a binary unit */
export function formatByteSize(
  value: number | null | undefined,
  options: ByteSizeFormatOptions = {},
): string {
  if (value === null || value === undefined || !Number.isFinite(value) || value < 0) {
    return options.fallback ?? "—"
  }

  const bytes = Math.trunc(value)
  if (bytes < BYTE_BASE) return `${formatNumberValue(bytes, {locale: options.locale})} B`

  const unitIndex = Math.min(
    BYTE_UNITS.length - 1,
    Math.floor(Math.log(bytes) / Math.log(BYTE_BASE)),
  )
  const scaled = bytes / BYTE_BASE ** unitIndex
  const maximumFractionDigits = options.maximumFractionDigits ?? (scaled < 10 ? 1 : 0)
  return `${formatNumberValue(scaled, {
    locale: options.locale,
    maximumFractionDigits,
  })} ${BYTE_UNITS[unitIndex]}`
}

export function ByteSize({
  value,
  fallback = "—",
  locale,
  maximumFractionDigits,
  ...props
}: ByteSizeProps) {
  const formatted = formatByteSize(value, {fallback: "", locale, maximumFractionDigits})
  if (!formatted) return fallback

  return (
    <data
      data-visual-dynamic="byte-size"
      data-visual-placeholder="<size>"
      {...props}
      value={value ?? undefined}
    >
      {formatted}
    </data>
  )
}

import type {HTMLAttributes, ReactNode} from "react"

import {
  formatNumberValue,
  type NumberValueFormatOptions,
  type NumberValueInput,
} from "../NumberValue"

export interface PercentageFormatOptions extends Omit<NumberValueFormatOptions, "signDisplay"> {
  readonly signDisplay?: NumberValueFormatOptions["signDisplay"]
}

export interface PercentageProps
  extends Omit<HTMLAttributes<HTMLDataElement>, "children" | "value">,
    Omit<PercentageFormatOptions, "fallback"> {
  readonly fallback?: ReactNode
  /** Treats `value` as a part of this total and converts the ratio to percentage points */
  readonly total?: number
  readonly value: NumberValueInput | null | undefined
}

/** Formats a value that already uses percentage points */
export function formatPercentage(
  value: NumberValueInput | null | undefined,
  options: PercentageFormatOptions = {},
): string {
  const formatted = formatNumberValue(value, {...options, fallback: ""})
  return formatted ? `${formatted}%` : (options.fallback ?? "—")
}

/** Formats a ratio as a percentage */
export function formatPercentageRatio(
  value: number,
  total: number,
  options: PercentageFormatOptions = {},
): string {
  const percentage =
    Number.isFinite(value) && Number.isFinite(total) && total > 0 ? (value / total) * 100 : 0
  return formatPercentage(percentage, options)
}

export function Percentage({value, total, fallback = "—", ...props}: PercentageProps) {
  const formatted =
    total === undefined
      ? formatPercentage(value, {...props, fallback: ""})
      : formatPercentageRatio(Number(value), total, {...props, fallback: ""})
  const {
    locale: _locale,
    maximumFractionDigits: _maximumFractionDigits,
    minimumFractionDigits: _minimumFractionDigits,
    roundingMode: _roundingMode,
    signDisplay: _signDisplay,
    useGrouping: _useGrouping,
    ...dataProps
  } = props
  if (!formatted) return fallback

  return (
    <data
      data-visual-dynamic="percentage"
      data-visual-placeholder="<percent>"
      {...dataProps}
      data-total={total}
      value={String(value)}
    >
      {formatted}
    </data>
  )
}

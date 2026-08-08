import type {HTMLAttributes, ReactNode} from "react"

import {formatNumberValue, NumberValue, type NumberValueInput} from "../NumberValue"

export interface CountValueFormatOptions {
  readonly fallback?: string
  readonly locale?: string
  readonly plural?: string
  readonly singular: string
  readonly useGrouping?: boolean
}

export interface CountValueProps
  extends Omit<HTMLAttributes<HTMLSpanElement>, "children">,
    Omit<CountValueFormatOptions, "fallback"> {
  readonly fallback?: ReactNode
  readonly value: NumberValueInput | null | undefined
}

/** Formats a count and selects its singular or plural label */
export function formatCountLabel(
  value: NumberValueInput | null | undefined,
  options: CountValueFormatOptions,
): string {
  const formatted = formatNumberValue(value, {
    fallback: "",
    locale: options.locale,
    maximumFractionDigits: 0,
    useGrouping: options.useGrouping ?? true,
  })
  if (!formatted) return options.fallback ?? "—"

  const singular = isSingularCount(value)
  return `${formatted} ${singular ? options.singular : (options.plural ?? `${options.singular}s`)}`
}

export function CountValue({
  value,
  singular,
  plural,
  fallback = "—",
  locale,
  useGrouping = true,
  ...props
}: CountValueProps) {
  if (!isIntegerValue(value)) return fallback

  return (
    <span data-visual-dynamic="count" data-visual-placeholder="<count>" {...props}>
      <NumberValue
        value={value}
        locale={locale}
        maximumFractionDigits={0}
        useGrouping={useGrouping}
      />{" "}
      {isSingularCount(value) ? singular : (plural ?? `${singular}s`)}
    </span>
  )
}

function isIntegerValue(value: NumberValueInput | null | undefined): value is NumberValueInput {
  if (typeof value === "bigint") return true
  if (typeof value === "number") return Number.isSafeInteger(value)
  return typeof value === "string" && /^[+-]?\d+$/.test(value.trim())
}

function isSingularCount(value: NumberValueInput | null | undefined): boolean {
  if (!isIntegerValue(value)) return false
  try {
    return BigInt(value) === 1n
  } catch {
    return false
  }
}

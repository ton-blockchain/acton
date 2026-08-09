import type {HTMLAttributes, ReactNode} from "react"

import {
  formatTokenAmount,
  type TokenAmountRoundingMode,
  type TokenAmountSignDisplay,
} from "../TokenAmount"

const MAX_DECIMAL_DIGITS = 36
const MAX_EXPONENT = 1000
const DECIMAL_VALUE_PATTERN = /^([+-]?)(\d+)(?:\.(\d*))?(?:[eE]([+-]?\d+))?$/

export type NumberValueInput = bigint | number | string

export interface NumberValueFormatOptions {
  readonly fallback?: string
  readonly locale?: string
  readonly maximumFractionDigits?: number
  readonly minimumFractionDigits?: number
  readonly roundingMode?: TokenAmountRoundingMode
  readonly signDisplay?: TokenAmountSignDisplay
  readonly useGrouping?: boolean
}

export interface NumberValueProps
  extends Omit<HTMLAttributes<HTMLDataElement>, "children" | "value">,
    Omit<NumberValueFormatOptions, "fallback"> {
  readonly fallback?: ReactNode
  readonly value: NumberValueInput | null | undefined
}

/** Formats a decimal value without converting string or bigint inputs to a float */
export function formatNumberValue(
  value: NumberValueInput | null | undefined,
  options: NumberValueFormatOptions = {},
): string {
  const normalized = normalizeDecimalValue(value)
  if (!normalized) return options.fallback ?? "—"

  return formatTokenAmount(normalized.rawValue, normalized.decimals, {
    fallback: options.fallback,
    locale: options.locale,
    maximumFractionDigits: options.maximumFractionDigits,
    minimumFractionDigits: options.minimumFractionDigits,
    roundingMode: options.roundingMode,
    showSymbol: false,
    signDisplay: options.signDisplay,
    useGrouping: options.useGrouping ?? true,
  })
}

export function NumberValue({
  value,
  fallback = "—",
  locale,
  maximumFractionDigits,
  minimumFractionDigits,
  roundingMode,
  signDisplay,
  useGrouping,
  ...props
}: NumberValueProps) {
  const formatted = formatNumberValue(value, {
    fallback: "",
    locale,
    maximumFractionDigits,
    minimumFractionDigits,
    roundingMode,
    signDisplay,
    useGrouping,
  })
  if (!formatted) return fallback

  return (
    <data
      data-visual-dynamic="number"
      data-visual-placeholder="<number>"
      {...props}
      value={String(value)}
    >
      {formatted}
    </data>
  )
}

interface NormalizedDecimalValue {
  readonly decimals: number
  readonly rawValue: bigint
}

function normalizeDecimalValue(
  value: NumberValueInput | null | undefined,
): NormalizedDecimalValue | undefined {
  if (typeof value === "bigint") return {decimals: 0, rawValue: value}
  if (typeof value === "number" && !Number.isFinite(value)) return undefined
  if (value === null || value === undefined) return undefined

  const normalized = String(value).trim()
  const match = DECIMAL_VALUE_PATTERN.exec(normalized)
  if (!match) return undefined

  const [, sign, whole, fraction = "", exponentText = "0"] = match
  const exponent = Number(exponentText)
  if (!Number.isSafeInteger(exponent) || Math.abs(exponent) > MAX_EXPONENT) return undefined

  const digits = `${whole}${fraction}`
  const decimalPosition = whole.length + exponent
  let rawDigits = digits
  let decimals = digits.length - decimalPosition

  if (decimalPosition <= 0) {
    rawDigits = `${"0".repeat(-decimalPosition)}${digits}`
    decimals = rawDigits.length
  } else if (decimalPosition >= digits.length) {
    rawDigits = `${digits}${"0".repeat(decimalPosition - digits.length)}`
    decimals = 0
  }

  if (decimals > MAX_DECIMAL_DIGITS) return undefined
  const trimmedDigits = rawDigits.replace(/^0+(?=\d)/, "") || "0"
  const rawValue = BigInt(
    `${sign === "-" && !/^0+$/.test(trimmedDigits) ? "-" : ""}${trimmedDigits}`,
  )
  return {decimals, rawValue}
}

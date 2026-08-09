import type {HTMLAttributes, ReactNode} from "react"

import {CopyInlineAction} from "../InlineActions"
import {Tooltip, type TooltipPlacement} from "../Tooltip"
import styles from "./TokenAmount.module.css"

const DEFAULT_TOKEN_DECIMALS = 9
const MAX_TOKEN_DECIMALS = 36

export type TokenAmountValue = bigint | number | string
export type TokenAmountDecimals = number | string | null | undefined
export type TokenAmountRoundingMode = "half-expand" | "truncate"
export type TokenAmountSignDisplay = "auto" | "always" | "except-zero" | "never"

/** Controls how an integer token value is presented with its decimal precision */
export interface TokenAmountFormatOptions {
  /** Text returned when the value is missing or is not an integer token value */
  readonly fallback?: string
  /** Locale used for decimal and optional grouping separators */
  readonly locale?: string
  /** Visible fractional token digits, from 0 to the token precision */
  readonly maximumFractionDigits?: number
  /** Trailing fractional digits preserved in the visible value */
  readonly minimumFractionDigits?: number
  /** How discarded raw units affect the last visible fractional digit */
  readonly roundingMode?: TokenAmountRoundingMode
  /** Show a lower bound instead of zero when compact precision hides a non-zero value */
  readonly showLessThanMinimum?: boolean
  /** Include the token symbol when one is provided */
  readonly showSymbol?: boolean
  /** Controls the sign without changing the raw token value */
  readonly signDisplay?: TokenAmountSignDisplay
  /** Token symbol appended to the formatted amount */
  readonly symbol?: string
  /** Add locale-aware separators to the whole token value */
  readonly useGrouping?: boolean
}

/** Props for a semantic token value with an exact-value tooltip */
export interface TokenAmountProps
  extends Omit<HTMLAttributes<HTMLDataElement>, "children" | "title" | "value">,
    Omit<TokenAmountFormatOptions, "fallback"> {
  /** Decimal precision of the raw integer value */
  readonly decimals: TokenAmountDecimals
  readonly fallback?: ReactNode
  /** Label for the integer value before decimal scaling */
  readonly rawUnitsLabel?: string
  /** Include the decimal precision in the tooltip */
  readonly showDecimalsInTooltip?: boolean
  /** Show the exact token amount and raw integer units with copy actions */
  readonly tooltip?: boolean
  readonly tooltipPlacement?: TooltipPlacement
  /** Integer raw token units, not an already scaled decimal value */
  readonly value: TokenAmountValue | null | undefined
}

interface FormattedTokenAmount {
  readonly decimals: number
  readonly rawValue: bigint
  readonly text: string
}

/**
 * Formats integer token units without converting them through a JavaScript
 * number. Invalid decimals use the Jetton-compatible default of 9.
 */
export function formatTokenAmount(
  value: TokenAmountValue | null | undefined,
  decimals: TokenAmountDecimals,
  options: TokenAmountFormatOptions = {},
): string {
  return formatTokenAmountValue(value, decimals, options)?.text ?? options.fallback ?? "—"
}

/**
 * Parses a non-negative decimal token amount into exact integer raw units.
 * Decimal metadata follows the same normalization rules as `formatTokenAmount`:
 * integer numbers and integer strings from 0 through 36 are accepted, and
 * invalid or missing metadata uses the Jetton-compatible default of 9.
 *
 * The parser does not use JavaScript floating-point numbers. It returns
 * `undefined` for an empty value, a negative value, exponent notation, or a
 * fractional part that is longer than the token precision.
 */
export function parseTokenAmount(value: string, decimals: TokenAmountDecimals): bigint | undefined {
  const normalized = value.trim()
  const normalizedDecimals = normalizeTokenDecimals(decimals)
  if (!/^(?:\d+(?:\.\d*)?|\.\d+)$/.test(normalized)) return undefined

  const [wholePart = "", fractionPart = ""] = normalized.split(".")
  if (
    fractionPart.length > normalizedDecimals ||
    (normalizedDecimals === 0 && normalized.includes("."))
  ) {
    return undefined
  }

  const scale = 10n ** BigInt(normalizedDecimals)
  const whole = BigInt(wholePart || "0") * scale
  const fraction = fractionPart ? BigInt(fractionPart.padEnd(normalizedDecimals, "0")) : 0n
  return whole + fraction
}

export function TokenAmount({
  value,
  decimals,
  locale,
  maximumFractionDigits,
  minimumFractionDigits,
  roundingMode,
  showLessThanMinimum,
  showSymbol,
  signDisplay,
  symbol,
  useGrouping,
  fallback = "—",
  rawUnitsLabel = "Raw units",
  showDecimalsInTooltip = true,
  tooltip = true,
  tooltipPlacement = "top",
  className,
  tabIndex,
  ...props
}: TokenAmountProps) {
  const options = {
    locale,
    maximumFractionDigits,
    minimumFractionDigits,
    roundingMode,
    showLessThanMinimum,
    showSymbol,
    signDisplay,
    symbol,
    useGrouping,
  }
  const formatted = formatTokenAmountValue(value, decimals, options)
  if (!formatted) return fallback

  const amount = (
    <data
      data-visual-dynamic="token-amount"
      data-visual-placeholder="<token>"
      {...props}
      className={
        [className, tooltip ? styles.trigger : undefined].filter(Boolean).join(" ") || undefined
      }
      tabIndex={tooltip ? (tabIndex ?? 0) : tabIndex}
      value={formatted.rawValue.toString()}
    >
      {formatted.text}
    </data>
  )

  return tooltip ? (
    <Tooltip
      content={
        <TokenAmountTooltip
          rawValue={formatted.rawValue}
          decimals={formatted.decimals}
          locale={locale}
          rawUnitsLabel={rawUnitsLabel}
          showDecimals={showDecimalsInTooltip}
          symbol={normalizedSymbol(symbol)}
        />
      }
      placement={tooltipPlacement}
      width="wide"
    >
      {amount}
    </Tooltip>
  ) : (
    amount
  )
}

function formatTokenAmountValue(
  value: TokenAmountValue | null | undefined,
  decimals: TokenAmountDecimals,
  options: TokenAmountFormatOptions,
): FormattedTokenAmount | undefined {
  const rawValue = tokenAmountValueToBigInt(value)
  if (rawValue === undefined) return undefined

  const normalizedDecimals = normalizeTokenDecimals(decimals)
  const maximumFractionDigits = clampFractionDigits(
    options.maximumFractionDigits ?? normalizedDecimals,
    normalizedDecimals,
  )
  const minimumFractionDigits = Math.min(
    clampFractionDigits(options.minimumFractionDigits ?? 0, normalizedDecimals),
    maximumFractionDigits,
  )
  const absolute = rawValue < 0n ? -rawValue : rawValue
  const base = 10n ** BigInt(normalizedDecimals)
  const roundingStep = 10n ** BigInt(normalizedDecimals - maximumFractionDigits)
  const roundedAbsolute =
    maximumFractionDigits === normalizedDecimals
      ? absolute
      : options.roundingMode === "truncate"
        ? (absolute / roundingStep) * roundingStep
        : ((absolute + roundingStep / 2n) / roundingStep) * roundingStep

  if (
    options.showLessThanMinimum &&
    absolute > 0n &&
    roundedAbsolute === 0n &&
    maximumFractionDigits < normalizedDecimals
  ) {
    const smallest = formatTokenAmount(roundingStep, normalizedDecimals, {
      locale: options.locale,
      maximumFractionDigits,
      minimumFractionDigits: maximumFractionDigits,
      roundingMode: "truncate",
      showSymbol: false,
    })
    const sign = amountSign(rawValue, options.signDisplay)
    const comparison = sign === "-" ? ">-" : `${sign}<`
    return {
      decimals: normalizedDecimals,
      rawValue,
      text: `${comparison}${smallest}${tokenSuffix(options)}`,
    }
  }

  const sign = amountSign(rawValue, options.signDisplay)
  const whole = roundedAbsolute / base
  const fraction = (roundedAbsolute % base)
    .toString()
    .padStart(normalizedDecimals, "0")
    .slice(0, maximumFractionDigits)
    .replace(/0+$/, "")
    .padEnd(minimumFractionDigits, "0")
  const wholeText = options.useGrouping
    ? new Intl.NumberFormat(options.locale, {maximumFractionDigits: 0}).format(whole)
    : whole.toString()
  const decimalSeparator = fraction ? localeDecimalSeparator(options.locale) : ""

  return {
    decimals: normalizedDecimals,
    rawValue,
    text: `${sign}${wholeText}${decimalSeparator}${fraction}${tokenSuffix(options)}`,
  }
}

function amountSign(value: bigint, signDisplay: TokenAmountSignDisplay = "auto"): string {
  if (signDisplay === "never") return ""
  if (value < 0n) return "-"
  if (signDisplay === "always" || (signDisplay === "except-zero" && value !== 0n)) return "+"
  return ""
}

function tokenAmountValueToBigInt(value: TokenAmountValue | null | undefined): bigint | undefined {
  if (typeof value === "bigint") return value
  if (typeof value === "number") {
    return Number.isSafeInteger(value) ? BigInt(value) : undefined
  }
  if (typeof value !== "string" || !/^[+-]?\d+$/.test(value.trim())) return undefined

  try {
    return BigInt(value.trim())
  } catch {
    return undefined
  }
}

function normalizeTokenDecimals(value: TokenAmountDecimals): number {
  const parsed =
    typeof value === "string" && /^\d+$/.test(value.trim()) ? Number(value.trim()) : value
  return typeof parsed === "number" &&
    Number.isInteger(parsed) &&
    parsed >= 0 &&
    parsed <= MAX_TOKEN_DECIMALS
    ? parsed
    : DEFAULT_TOKEN_DECIMALS
}

function clampFractionDigits(value: number, decimals: number): number {
  if (!Number.isFinite(value)) return 0
  return Math.max(0, Math.min(decimals, Math.trunc(value)))
}

function localeDecimalSeparator(locale: string | undefined): string {
  return (
    new Intl.NumberFormat(locale).formatToParts(1.1).find(part => part.type === "decimal")?.value ??
    "."
  )
}

function normalizedSymbol(symbol: string | undefined): string | undefined {
  const normalized = symbol?.trim()
  return normalized || undefined
}

function tokenSuffix(options: TokenAmountFormatOptions): string {
  const symbol = normalizedSymbol(options.symbol)
  return options.showSymbol === false || !symbol ? "" : ` ${symbol}`
}

function TokenAmountTooltip({
  rawValue,
  decimals,
  locale,
  rawUnitsLabel,
  showDecimals,
  symbol,
}: {
  readonly rawValue: bigint
  readonly decimals: number
  readonly locale?: string
  readonly rawUnitsLabel: string
  readonly showDecimals: boolean
  readonly symbol?: string
}) {
  const exactAmount = formatTokenAmount(rawValue, decimals, {locale, showSymbol: false})
  const decimalPrecision = decimals.toString()
  const rawUnits = rawValue.toString()
  const amountLabel = symbol ?? "Amount"
  const amountCopySubject = symbol ? `${symbol} amount` : "Token amount"
  const rawUnitsCopySubject = rawUnitsLabel.toLocaleLowerCase(locale)

  return (
    <span className={styles.tooltip}>
      <span className={styles.tooltipRow}>
        <span>{amountLabel}</span>
        <span className={styles.tooltipCopyValue}>
          <code>{exactAmount}</code>
          <CopyInlineAction
            copiedLabel={`${amountCopySubject} copied`}
            label={`Copy ${amountCopySubject}`}
            size="compact"
            value={exactAmount}
          />
        </span>
      </span>
      <span className={styles.tooltipRow}>
        <span>{rawUnitsLabel}</span>
        <span className={styles.tooltipCopyValue}>
          <code>{rawUnits}</code>
          <CopyInlineAction
            copiedLabel={`${rawUnitsLabel} copied`}
            label={`Copy ${rawUnitsCopySubject}`}
            size="compact"
            value={rawUnits}
          />
        </span>
      </span>
      {showDecimals && (
        <span className={styles.tooltipRow}>
          <span>Decimals</span>
          <span className={styles.tooltipCopyValue}>
            <code>{decimalPrecision}</code>
            <CopyInlineAction
              copiedLabel="Token decimals copied"
              label="Copy token decimals"
              size="compact"
              value={decimalPrecision}
            />
          </span>
        </span>
      )}
    </span>
  )
}

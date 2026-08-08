import type {ReactNode} from "react"

import {
  formatTokenAmount,
  parseTokenAmount,
  TokenAmount,
  type TokenAmountFormatOptions,
  type TokenAmountProps,
  type TokenAmountRoundingMode,
  type TokenAmountSignDisplay,
  type TokenAmountValue,
} from "../TokenAmount/TokenAmount"

const NANOGRAM_DECIMALS = 9

export type GramAmountValue = TokenAmountValue
export type GramAmountRoundingMode = TokenAmountRoundingMode
export type GramAmountSignDisplay = TokenAmountSignDisplay

/** Controls how an integer nanogram value is presented as GRAM */
export interface GramAmountFormatOptions
  extends Omit<TokenAmountFormatOptions, "showSymbol" | "symbol"> {
  /** Include the `GRAM` suffix; disable it for inputs that provide their own suffix */
  readonly showUnit?: boolean
}

/** Props for a semantic GRAM value with an exact-value tooltip */
export interface GramAmountProps
  extends Omit<
      TokenAmountProps,
      "decimals" | "fallback" | "rawUnitsLabel" | "showDecimalsInTooltip" | "showSymbol" | "symbol"
    >,
    Omit<GramAmountFormatOptions, "fallback"> {
  readonly fallback?: ReactNode
  /** Integer nanograms, not a decimal GRAM value */
  readonly value: GramAmountValue | null | undefined
}

/**
 * Formats an integer nanogram value as GRAM without converting it through a
 * JavaScript number. The default output keeps all significant nanogram digits
 * and appends the `GRAM` unit.
 */
export function formatGramAmount(
  value: GramAmountValue | null | undefined,
  options: GramAmountFormatOptions = {},
): string {
  return formatTokenAmount(value, NANOGRAM_DECIMALS, {
    ...options,
    showSymbol: options.showUnit !== false,
    symbol: "GRAM",
  })
}

/** Parses a non-negative decimal GRAM amount into exact integer nanograms */
export function parseGramAmount(value: string): bigint | undefined {
  return parseTokenAmount(value, NANOGRAM_DECIMALS)
}

export function GramAmount({showUnit, ...props}: GramAmountProps) {
  return (
    <TokenAmount
      data-visual-dynamic="gram-amount"
      data-visual-placeholder="<gram>"
      {...props}
      decimals={NANOGRAM_DECIMALS}
      rawUnitsLabel="Nanograms"
      showDecimalsInTooltip={false}
      showSymbol={showUnit !== false}
      symbol="GRAM"
    />
  )
}

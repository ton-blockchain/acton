import {describe, expect, test} from "bun:test"
import {createElement} from "react"
import {renderToStaticMarkup} from "react-dom/server"

import {TokenAmount, formatTokenAmount, parseTokenAmount} from "../src/components/TokenAmount"

describe("token amounts", () => {
  test("parses decimal token amounts into exact raw units", () => {
    expect({
      zeroDecimals: parseTokenAmount("42", 0),
      stringDecimals: parseTokenAmount("1.234567", "6"),
      leadingFraction: parseTokenAmount(".5", 9),
      trailingDecimal: parseTokenAmount("12.", 9),
      zero: parseTokenAmount("0", 9),
      large: parseTokenAmount("123456789012345678901.123456789012345678", 18),
    }).toMatchInlineSnapshot(`
      {
        "large": 123456789012345678901123456789012345678n,
        "leadingFraction": 500000000n,
        "stringDecimals": 1234567n,
        "trailingDecimal": 12000000000n,
        "zero": 0n,
        "zeroDecimals": 42n,
      }
    `)
  })

  test("rejects token amounts that cannot be represented exactly", () => {
    expect({
      empty: parseTokenAmount("", 9),
      exponent: parseTokenAmount("1e9", 9),
      negative: parseTokenAmount("-1", 9),
      separatorWithoutDigits: parseTokenAmount(".", 9),
      fractionWithZeroDecimals: parseTokenAmount("1.", 0),
      tooPrecise: parseTokenAmount("0.0000001", "6"),
    }).toMatchInlineSnapshot(`
      {
        "empty": undefined,
        "exponent": undefined,
        "fractionWithZeroDecimals": undefined,
        "negative": undefined,
        "separatorWithoutDigits": undefined,
        "tooPrecise": undefined,
      }
    `)
  })

  test("uses the same decimal metadata fallback for parsing and formatting", () => {
    expect({
      missing: parseTokenAmount("1.5", undefined),
      invalidString: parseTokenAmount("1.5", "not-a-number"),
      fractionalNumber: parseTokenAmount("1.5", 1.5),
      excessive: parseTokenAmount("1.5", 37),
    }).toMatchInlineSnapshot(`
      {
        "excessive": 1500000000n,
        "fractionalNumber": 1500000000n,
        "invalidString": 1500000000n,
        "missing": 1500000000n,
      }
    `)
  })

  test("formats exact raw token units without losing precision", () => {
    expect({
      zeroDecimals: formatTokenAmount("42", 0, {symbol: "NFT"}),
      usdt: formatTokenAmount("1234567", "6", {symbol: "USDT"}),
      act: formatTokenAmount("1000000000000000", 9, {symbol: "ACT"}),
      negative: formatTokenAmount("-250000", 6, {symbol: "USD₮"}),
      large: formatTokenAmount("123456789012345678901234567890", 18, {symbol: "JET"}),
    }).toMatchInlineSnapshot(`
      {
        "act": "1000000 ACT",
        "large": "123456789012.34567890123456789 JET",
        "negative": "-0.25 USD₮",
        "usdt": "1.234567 USDT",
        "zeroDecimals": "42 NFT",
      }
    `)
  })

  test("supports grouping, compact precision, rounding, and signs", () => {
    expect({
      grouped: formatTokenAmount("123456789000000", 6, {
        locale: "en-US",
        maximumFractionDigits: 2,
        symbol: "USDT",
        useGrouping: true,
      }),
      lowerBound: formatTokenAmount("1", 9, {
        maximumFractionDigits: 4,
        showLessThanMinimum: true,
        symbol: "ACT",
      }),
      negativeLowerBound: formatTokenAmount("-1", 9, {
        maximumFractionDigits: 4,
        showLessThanMinimum: true,
        symbol: "ACT",
      }),
      signlessNegativeLowerBound: formatTokenAmount("-1", 9, {
        maximumFractionDigits: 4,
        showLessThanMinimum: true,
        signDisplay: "never",
        symbol: "ACT",
      }),
      rounded: formatTokenAmount("1999999", 6, {
        maximumFractionDigits: 0,
        symbol: "USDT",
      }),
      truncated: formatTokenAmount("1999999", 6, {
        maximumFractionDigits: 0,
        roundingMode: "truncate",
        symbol: "USDT",
      }),
      signed: formatTokenAmount("1000000", 6, {
        signDisplay: "always",
        symbol: "USDT",
      }),
    }).toMatchInlineSnapshot(`
      {
        "grouped": "123,456,789 USDT",
        "lowerBound": "<0.0001 ACT",
        "negativeLowerBound": ">-0.0001 ACT",
        "rounded": "2 USDT",
        "signed": "+1 USDT",
        "signlessNegativeLowerBound": "<0.0001 ACT",
        "truncated": "1 USDT",
      }
    `)
  })

  test("uses nine decimals as the safe fallback for invalid metadata", () => {
    expect({
      fractionalDecimals: formatTokenAmount("1500000000", 1.5, {symbol: "JET"}),
      negativeDecimals: formatTokenAmount("1500000000", -1, {symbol: "JET"}),
      excessiveDecimals: formatTokenAmount("1500000000", 37, {symbol: "JET"}),
      invalidValue: formatTokenAmount("1.5", 9, {fallback: "invalid", symbol: "JET"}),
      unsafeNumber: formatTokenAmount(Number.MAX_SAFE_INTEGER + 1, 9, {
        fallback: "invalid",
        symbol: "JET",
      }),
    }).toMatchInlineSnapshot(`
      {
        "excessiveDecimals": "1.5 JET",
        "fractionalDecimals": "1.5 JET",
        "invalidValue": "invalid",
        "negativeDecimals": "1.5 JET",
        "unsafeNumber": "invalid",
      }
    `)
  })

  test("renders a semantic data element with the raw integer value", () => {
    expect(
      renderToStaticMarkup(
        createElement(TokenAmount, {
          decimals: 6,
          symbol: "USDT",
          tooltip: false,
          value: "1250000",
        }),
      ),
    ).toMatchInlineSnapshot(
      `"<data data-visual-dynamic="token-amount" data-visual-placeholder="&lt;token&gt;" value="1250000">1.25 USDT</data>"`,
    )
  })
})

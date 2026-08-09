import {describe, expect, test} from "bun:test"
import {createElement} from "react"
import {renderToStaticMarkup} from "react-dom/server"

import {GramAmount, formatGramAmount, parseGramAmount} from "../src/components/GramAmount"

describe("GRAM amounts", () => {
  test("parses decimal GRAM amounts into exact nanograms", () => {
    expect({
      zero: parseGramAmount("0"),
      oneNanogram: parseGramAmount("0.000000001"),
      leadingFraction: parseGramAmount(".5"),
      large: parseGramAmount("123456789012345678901.123456789"),
      tooPrecise: parseGramAmount("0.0000000001"),
      negative: parseGramAmount("-1"),
    }).toMatchInlineSnapshot(`
      {
        "large": 123456789012345678901123456789n,
        "leadingFraction": 500000000n,
        "negative": undefined,
        "oneNanogram": 1n,
        "tooPrecise": undefined,
        "zero": 0n,
      }
    `)
  })

  test("formats exact nanogram values without losing precision", () => {
    expect({
      zero: formatGramAmount(0n),
      oneNanogram: formatGramAmount(1n),
      fractional: formatGramAmount(1_234_567_890n),
      negative: formatGramAmount(-250_000_000n),
      large: formatGramAmount("123456789012345678901234567890"),
      invalid: formatGramAmount("not-an-amount"),
    }).toMatchInlineSnapshot(`
      {
        "fractional": "1.23456789 GRAM",
        "invalid": "—",
        "large": "123456789012345678901.23456789 GRAM",
        "negative": "-0.25 GRAM",
        "oneNanogram": "0.000000001 GRAM",
        "zero": "0 GRAM",
      }
    `)
  })

  test("supports grouping, precision, rounding, and signs", () => {
    expect({
      grouped: formatGramAmount(1_234_567_890_000_000n, {
        locale: "en-US",
        maximumFractionDigits: 2,
        useGrouping: true,
      }),
      rounded: formatGramAmount(1_235_000_000n, {maximumFractionDigits: 2}),
      truncated: formatGramAmount(1_239_000_000n, {
        maximumFractionDigits: 2,
        roundingMode: "truncate",
      }),
      minimumDigits: formatGramAmount(1_000_000_000n, {
        maximumFractionDigits: 4,
        minimumFractionDigits: 2,
      }),
      positive: formatGramAmount(1_000_000_000n, {signDisplay: "except-zero"}),
      hiddenSign: formatGramAmount(-1_000_000_000n, {signDisplay: "never"}),
    }).toMatchInlineSnapshot(`
      {
        "grouped": "1,234,567.89 GRAM",
        "hiddenSign": "1 GRAM",
        "minimumDigits": "1.00 GRAM",
        "positive": "+1 GRAM",
        "rounded": "1.24 GRAM",
        "truncated": "1.23 GRAM",
      }
    `)
  })

  test("can show a compact lower bound for non-zero values", () => {
    expect(
      formatGramAmount(1n, {
        maximumFractionDigits: 4,
        showLessThanMinimum: true,
      }),
    ).toBe("<0.0001 GRAM")
  })

  test("rejects values that cannot represent exact integer nanograms", () => {
    expect({
      empty: formatGramAmount("", {fallback: "invalid"}),
      exponent: formatGramAmount("1e9", {fallback: "invalid"}),
      fractionalNumber: formatGramAmount(1.5, {fallback: "invalid"}),
      unsafeNumber: formatGramAmount(Number.MAX_SAFE_INTEGER + 1, {fallback: "invalid"}),
      signedString: formatGramAmount(" -1500000000 "),
    }).toMatchInlineSnapshot(`
      {
        "empty": "invalid",
        "exponent": "invalid",
        "fractionalNumber": "invalid",
        "signedString": "-1.5 GRAM",
        "unsafeNumber": "invalid",
      }
    `)
  })

  test("rounds across the whole GRAM boundary without losing bigint precision", () => {
    expect({
      rounded: formatGramAmount(1_999_999_999n, {maximumFractionDigits: 0}),
      truncated: formatGramAmount(1_999_999_999n, {
        maximumFractionDigits: 0,
        roundingMode: "truncate",
      }),
    }).toMatchInlineSnapshot(`
      {
        "rounded": "2 GRAM",
        "truncated": "1 GRAM",
      }
    `)
  })

  test("renders a semantic data element", () => {
    expect({
      default: renderToStaticMarkup(
        createElement(GramAmount, {
          value: 1_250_000_000n,
          maximumFractionDigits: 2,
          tooltip: false,
        }),
      ),
      hiddenSign: renderToStaticMarkup(
        createElement(GramAmount, {
          value: -1_250_000_000n,
          signDisplay: "never",
          tooltip: false,
        }),
      ),
    }).toMatchInlineSnapshot(`
      {
        "default": "<data data-visual-dynamic="gram-amount" data-visual-placeholder="&lt;gram&gt;" value="1250000000">1.25 GRAM</data>",
        "hiddenSign": "<data data-visual-dynamic="gram-amount" data-visual-placeholder="&lt;gram&gt;" value="-1250000000">1.25 GRAM</data>",
      }
    `)
  })
})

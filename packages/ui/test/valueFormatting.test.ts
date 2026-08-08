import {describe, expect, test} from "bun:test"
import {createElement} from "react"
import {renderToStaticMarkup} from "react-dom/server"

import {
  BooleanValue,
  ByteSize,
  CountValue,
  NumberValue,
  Percentage,
  SourceLocationValue,
  TechnicalValue,
  formatByteSize,
  formatCompilerLabel,
  formatCountLabel,
  formatNumberValue,
  formatOpcode,
  formatPercentage,
  formatPercentageRatio,
  formatSourceLocation,
  formatSourcePath,
  humanizeIdentifier,
  shortenMiddle,
  truncateEnd,
} from "../src"

describe("shared value formatting", () => {
  test("shortens identifiers without changing short values", () => {
    expect({
      default: shortenMiddle("1234567890abcdefghijkl"),
      exact: shortenMiddle("1234567890", {start: 4, end: 3}),
      maximum: shortenMiddle("blackmarket-dot-tg-exch.ton", {maxLength: 20}),
    }).toMatchInlineSnapshot(`
      {
        "default": "123456…ghijkl",
        "exact": "1234…890",
        "maximum": "blackmarke…-exch.ton",
      }
    `)
  })

  test("truncates text at the end within the requested length", () => {
    expect({
      exact: truncateEnd("1234567890", 7),
      ascii: truncateEnd("1234567890", 7, "..."),
      short: truncateEnd("123", 7),
      empty: truncateEnd("123", 0),
      separatorOnly: truncateEnd("123", 1, "..."),
    }).toMatchInlineSnapshot(`
      {
        "ascii": "1234...",
        "empty": "",
        "exact": "123456…",
        "separatorOnly": ".",
        "short": "123",
      }
    `)
  })

  test("formats machine labels and compiler labels", () => {
    expect({
      compiler: formatCompilerLabel({language: " tolk ", version: " 1.4.2 "}),
      fallback: formatCompilerLabel({language: "", version: "1"}, "Unavailable"),
      label: humanizeIdentifier("validator_network"),
      sentence: humanizeIdentifier("pending-message", {capitalize: true}),
    }).toMatchInlineSnapshot(`
      {
        "compiler": "tolk 1.4.2",
        "fallback": "Unavailable",
        "label": "validator network",
        "sentence": "Pending message",
      }
    `)
  })

  test("formats exact decimal strings and large integers", () => {
    expect({
      decimal: formatNumberValue("12345678901234567.125", {locale: "en-US"}),
      exponent: formatNumberValue("1.25e6", {locale: "en-US"}),
      large: formatNumberValue(123456789012345678901234567890n, {locale: "en-US"}),
      rounded: formatNumberValue("1.9999", {maximumFractionDigits: 2}),
      signed: formatNumberValue("42", {signDisplay: "always"}),
    }).toMatchInlineSnapshot(`
      {
        "decimal": "12,345,678,901,234,567.125",
        "exponent": "1,250,000",
        "large": "123,456,789,012,345,678,901,234,567,890",
        "rounded": "2",
        "signed": "+42",
      }
    `)
  })

  test("formats byte sizes with one unit contract", () => {
    expect({
      bytes: formatByteSize(512),
      kilobytes: formatByteSize(1536),
      megabytes: formatByteSize(12 * 1024 * 1024),
      terabytes: formatByteSize(1.25 * 1024 ** 4),
    }).toMatchInlineSnapshot(`
      {
        "bytes": "512 B",
        "kilobytes": "1.5 KB",
        "megabytes": "12 MB",
        "terabytes": "1.3 TB",
      }
    `)
  })

  test("formats percentages and ratios", () => {
    expect({
      direct: formatPercentage("9.45", {maximumFractionDigits: 1}),
      ratio: formatPercentageRatio(3, 8, {minimumFractionDigits: 1}),
      zeroTotal: formatPercentageRatio(3, 0),
    }).toMatchInlineSnapshot(`
      {
        "direct": "9.5%",
        "ratio": "37.5%",
        "zeroTotal": "0%",
      }
    `)
  })

  test("formats count labels", () => {
    expect({
      one: formatCountLabel(1, {singular: "transaction"}),
      many: formatCountLabel(1200, {singular: "transaction"}),
      irregular: formatCountLabel(2, {singular: "entry", plural: "entries"}),
    }).toMatchInlineSnapshot(`
      {
        "irregular": "2 entries",
        "many": "1,200 transactions",
        "one": "1 transaction",
      }
    `)
  })

  test("formats source paths and locations", () => {
    const value = {file: "/workspace/project/tests/unit/wallet.test.tolk", line: 42, column: 7}
    expect({
      relative: formatSourcePath(value.file, {projectRoot: "/workspace/project"}),
      shortened: formatSourceLocation(value, {maxSegments: 2}),
      windows: formatSourcePath("C:\\project\\tests\\wallet.test.tolk", {
        projectRoot: "C:\\project",
      }),
    }).toMatchInlineSnapshot(`
      {
        "relative": "tests/unit/wallet.test.tolk",
        "shortened": "…/unit/wallet.test.tolk:42:7",
        "windows": "tests/wallet.test.tolk",
      }
    `)
  })

  test("formats opcodes as unsigned 32-bit values", () => {
    expect({
      decimal: formatOpcode("1"),
      hexadecimal: formatOpcode("0x7362d09c"),
      negative: formatOpcode(-1),
      invalid: formatOpcode("nope"),
    }).toMatchInlineSnapshot(`
      {
        "decimal": "0x00000001",
        "hexadecimal": "0x7362d09c",
        "invalid": undefined,
        "negative": "0xffffffff",
      }
    `)
  })

  test("renders semantic value components", () => {
    expect({
      boolean: renderToStaticMarkup(createElement(BooleanValue, {value: true})),
      bytes: renderToStaticMarkup(createElement(ByteSize, {value: 1536})),
      count: renderToStaticMarkup(createElement(CountValue, {value: 2, singular: "transaction"})),
      number: renderToStaticMarkup(createElement(NumberValue, {value: "12345678901234567"})),
      percentage: renderToStaticMarkup(createElement(Percentage, {value: "12.5"})),
      percentageRatio: renderToStaticMarkup(
        createElement(Percentage, {value: 1, total: 8, maximumFractionDigits: 1}),
      ),
      source: renderToStaticMarkup(
        createElement(SourceLocationValue, {
          value: {file: "/workspace/tests/a.tolk", line: 2, column: 4},
          maxSegments: 2,
        }),
      ),
      technical: renderToStaticMarkup(
        createElement(TechnicalValue, {value: "1234567890abcdef", tooltip: false}),
      ),
    }).toMatchSnapshot()
  })
})

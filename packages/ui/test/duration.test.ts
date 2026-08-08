import {describe, expect, test} from "bun:test"
import {createElement} from "react"
import {renderToStaticMarkup} from "react-dom/server"

import {
  Duration,
  formatDuration,
  formatRecurringPeriod,
  formatSchedulePeriod,
} from "../src/components/Duration"
import {DAY_SECONDS} from "../src/lib/time"

describe("duration", () => {
  test("exports the shared number of seconds per day", () => {
    expect(DAY_SECONDS).toBe(86_400)
  })

  test("formats shared duration presets", () => {
    expect({
      compactSeconds: formatDuration(42),
      compactMinutes: formatDuration(300),
      elapsed: formatDuration(125, {display: "elapsed"}),
      human: formatDuration(125, {display: "human"}),
      latency: formatDuration(1_234_567, {display: "latency", unit: "nanoseconds"}),
      parts: formatDuration(90_061, {display: "parts", maxParts: 2, sign: "always"}),
      precise: formatDuration(0.125, {display: "precise", unit: "milliseconds"}),
      readable: formatDuration(90_061, {display: "readable", sign: "always"}),
      runtime: formatDuration(65_400, {display: "runtime", unit: "milliseconds"}),
      startup: formatDuration(1250, {display: "startup", unit: "milliseconds"}),
      invalid: formatDuration(Number.NaN),
    }).toMatchInlineSnapshot(`
      {
        "compactMinutes": "5m",
        "compactSeconds": "42s",
        "elapsed": "2m 05s",
        "human": "2 min 5 sec",
        "invalid": "—",
        "latency": "1.23 ms",
        "parts": "+1d 1h",
        "precise": "125µs",
        "readable": "+1 day 1 hour 1 minute 1 second",
        "runtime": "1m 5s",
        "startup": "1.3 s",
      }
    `)
  })

  test("formats schedule periods", () => {
    expect({
      day: formatSchedulePeriod(DAY_SECONDS),
      days: formatSchedulePeriod(2 * DAY_SECONDS),
      hour: formatSchedulePeriod(3600),
      hours: formatSchedulePeriod(2 * 3600),
      minute: formatSchedulePeriod(60),
      minutes: formatSchedulePeriod(5 * 60),
      zero: formatSchedulePeriod(0),
      invalid: formatSchedulePeriod(Number.NaN),
      seconds: formatSchedulePeriod(90),
    }).toMatchInlineSnapshot(`
      {
        "day": "1 day",
        "days": "2 days",
        "hour": "1 hour",
        "hours": "2 hours",
        "invalid": "—",
        "minute": "1 minute",
        "minutes": "5 minutes",
        "seconds": "90 seconds",
        "zero": "0 seconds",
      }
    `)
  })

  test("formats recurring periods", () => {
    expect({
      hour: formatRecurringPeriod(3600, {unit: "seconds"}),
      day: formatRecurringPeriod(86_400_000),
      minutes: formatRecurringPeriod(90 * 60_000),
      roundedMinutes: formatRecurringPeriod(90_000),
      minimumMinute: formatRecurringPeriod(1000),
      invalid: formatRecurringPeriod(Number.NaN),
    }).toMatchInlineSnapshot(`
      {
        "day": "per day",
        "hour": "per hour",
        "invalid": "—",
        "minimumMinute": "every 1 minute",
        "minutes": "every 90 minutes",
        "roundedMinutes": "every 2 minutes",
      }
    `)
  })

  test("formats duration boundaries, signs, and source units", () => {
    expect({
      compactMinuteBoundary: formatDuration(60),
      compactHourBoundary: formatDuration(3600),
      compactDayBoundary: formatDuration(86_400),
      elapsedUnderMinute: formatDuration(59.9, {display: "elapsed"}),
      negativeParts: formatDuration(-3661, {display: "parts", maxParts: 3}),
      noSign: formatDuration(-3600, {display: "compact", sign: "never"}),
      milliseconds: formatDuration(3_600_000, {unit: "milliseconds"}),
      nanoseconds: formatDuration(3_600_000_000_000, {unit: "nanoseconds"}),
      zeroReadable: formatDuration(0, {display: "readable"}),
    }).toMatchInlineSnapshot(`
      {
        "compactDayBoundary": "1d",
        "compactHourBoundary": "1h",
        "compactMinuteBoundary": "1m",
        "elapsedUnderMinute": "59s",
        "milliseconds": "1h",
        "nanoseconds": "1h",
        "negativeParts": "-1h 1min 1s",
        "noSign": "1h",
        "zeroReadable": "0 seconds",
      }
    `)
  })

  test("renders duration metadata on the component", () => {
    const markup = renderToStaticMarkup(
      createElement(Duration, {
        display: "compact",
        value: 3600,
      }),
    ).replace(/\s+(?:tabindex|id|data-base-ui-tooltip-trigger)(?:="[^"]*")?/g, "")

    expect(markup).toMatchInlineSnapshot(
      `"<span data-visual-dynamic="duration" data-visual-placeholder="&lt;duration&gt;">1h</span>"`,
    )
  })
})

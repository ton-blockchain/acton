import {version as bunVersion} from "bun"
import {describe, expect, setSystemTime, test} from "bun:test"
import {createElement} from "react"
import {renderToStaticMarkup} from "react-dom/server"

import {
  DateTime,
  RelativeTime,
  dateTimeValueToDate,
  formatDateTime,
  formatDateTimeLocalInput,
  formatRelativeDateTime,
  formatTimeUntil,
} from "../src/components/DateTime"

const NOW = Date.UTC(2026, 7, 5, 12, 0)
const DATE = Date.UTC(2026, 7, 5, 10, 30, 15)

// Bun 1.3.14 uses "at" on macOS; the issue reports this fixed in Bun 1.4.0.
// https://github.com/oven-sh/bun/issues/6056
const DATE_TIME_SEPARATOR =
  bunVersion === "1.3.14" && navigator.platform.startsWith("Mac") ? " at " : ", "

function expectedDateTime(date: string, time: string): string {
  return `${date}${DATE_TIME_SEPARATOR}${time}`
}

describe("date and time", () => {
  test("formats shared date and time presets", () => {
    expect({
      date: formatDateTime(DATE, {display: "date", locale: "en-US", timeZone: "UTC"}),
      dateLong: formatDateTime(DATE, {
        display: "date-long",
        locale: "en-US",
        timeZone: "UTC",
      }),
      dateNumeric: formatDateTime(DATE, {
        display: "date-numeric",
        timeZone: "UTC",
      }),
      dateDayMonth: formatDateTime(DATE, {
        display: "date-day-month",
        locale: "en-US",
        timeZone: "UTC",
      }),
      dateTimeDayMonth: formatDateTime(DATE, {
        display: "date-time-day-month",
        locale: "en-US",
        timeZone: "UTC",
      }),
      dateTimeDayMonthShort: formatDateTime(DATE, {
        display: "date-time-day-month-short",
        locale: "en-US",
        timeZone: "UTC",
      }),
      dateTime: formatDateTime(DATE, {
        display: "date-time",
        locale: "en-US",
        timeZone: "UTC",
      }),
      dateTimeSeconds: formatDateTime(DATE, {
        display: "date-time-seconds",
        locale: "en-US",
        timeZone: "UTC",
      }),
      dateTimeNumeric: formatDateTime(DATE, {
        display: "date-time-numeric",
        timeZone: "UTC",
      }),
      dateTimeNumericSeconds: formatDateTime(DATE, {
        display: "date-time-numeric-seconds",
        timeZone: "UTC",
      }),
      time: formatDateTime(DATE, {display: "time", locale: "en-US", timeZone: "UTC"}),
      compact: formatDateTime(DATE, {
        display: "compact",
        locale: "en-US",
        now: NOW,
        timeZone: "UTC",
      }),
      smartToday: formatDateTime(DATE, {
        display: "smart",
        locale: "en-US",
        now: NOW,
        timeZone: "UTC",
      }),
      invalid: formatDateTime("not-a-date"),
    }).toEqual({
      compact: "5 Aug, 10:30",
      date: "Aug 5, 2026",
      dateDayMonth: "05 Aug 2026",
      dateLong: "August 5, 2026",
      dateNumeric: "05.08.2026",
      dateTime: expectedDateTime("Aug 5, 2026", "10:30"),
      dateTimeDayMonth: "05 Aug 2026, 10:30",
      dateTimeDayMonthShort: "05 Aug, 10:30",
      dateTimeNumeric: "05.08.2026, 10:30",
      dateTimeNumericSeconds: "05.08.2026, 10:30:15",
      dateTimeSeconds: expectedDateTime("Aug 5, 2026", "10:30:15"),
      invalid: "—",
      smartToday: "10:30",
      time: "10:30",
    })
  })

  test("always uses the 24-hour clock", () => {
    expect(
      formatDateTime("2026-08-01T22:40:00.000Z", {
        display: "date-time",
        locale: "en-US",
        timeZone: "UTC",
      }),
    ).toBe(expectedDateTime("Aug 1, 2026", "22:40"))
  })

  test("keeps numeric dates stable across locales", () => {
    expect({
      arabic: formatDateTime(DATE, {
        display: "date-time-numeric-seconds",
        locale: "ar",
        timeZone: "UTC",
      }),
      german: formatDateTime(DATE, {
        display: "date-numeric",
        locale: "de",
        timeZone: "UTC",
      }),
      us: formatDateTime(DATE, {
        display: "date-numeric",
        locale: "en-US",
        timeZone: "UTC",
      }),
    }).toMatchInlineSnapshot(`
      {
        "arabic": "05.08.2026, 10:30:15",
        "german": "05.08.2026",
        "us": "05.08.2026",
      }
    `)
  })

  test("uses the real clock correctly for second-based values", () => {
    setSystemTime(new Date(NOW))
    try {
      expect(formatRelativeDateTime(DATE / 1000, {unit: "seconds"})).toBe("1h ago")
    } finally {
      setSystemTime()
    }
  })

  test("formats relative past, future, and hybrid values", () => {
    expect({
      now: formatRelativeDateTime(NOW, {now: NOW}),
      seconds: formatRelativeDateTime(NOW - 15_000, {now: NOW}),
      minutes: formatRelativeDateTime(NOW - 5 * 60_000, {now: NOW}),
      future: formatRelativeDateTime(NOW + 61_000, {now: NOW}),
      hybrid: formatRelativeDateTime(NOW - 2 * 86_400_000, {
        locale: "en-US",
        mode: "hybrid",
        now: NOW,
        timeZone: "UTC",
      }),
      secondsUnit: formatRelativeDateTime(DATE / 1000, {
        now: NOW / 1000,
        unit: "seconds",
      }),
    }).toMatchInlineSnapshot(`
      {
        "future": "in 2m",
        "hybrid": "3 Aug, 12:00",
        "minutes": "5m ago",
        "now": "right now",
        "seconds": "15s ago",
        "secondsUnit": "1h ago",
      }
    `)
  })

  test("formats relative time boundaries", () => {
    expect({
      pastMinute: formatRelativeDateTime(NOW - 60_000, {now: NOW}),
      futureHour: formatRelativeDateTime(NOW + 3_600_000, {now: NOW}),
      pastDay: formatRelativeDateTime(NOW - 86_400_000, {now: NOW}),
      pastWeek: formatRelativeDateTime(NOW - 604_800_000, {now: NOW}),
      pastMonth: formatRelativeDateTime(NOW - 2_629_800_000, {now: NOW}),
      pastYear: formatRelativeDateTime(NOW - 31_557_600_000, {now: NOW}),
    }).toMatchInlineSnapshot(`
      {
        "futureHour": "in 1h",
        "pastDay": "1d ago",
        "pastMinute": "1m ago",
        "pastMonth": "1mo ago",
        "pastWeek": "1w ago",
        "pastYear": "1y ago",
      }
    `)
  })

  test("formats time until a timestamp", () => {
    expect({
      exactWeek: formatTimeUntil(7 * 86_400, 0),
      weeks: formatTimeUntil(15 * 86_400, 0),
      exactDay: formatTimeUntil(86_400, 0),
      days: formatTimeUntil(2 * 86_400, 0),
      almostDay: formatTimeUntil(86_399, 0),
      exactHour: formatTimeUntil(3600, 0),
      hours: formatTimeUntil(2 * 3600, 0),
      past: formatTimeUntil(0, 30),
      soon: formatTimeUntil(30, 0),
    }).toMatchInlineSnapshot(`
      {
        "almostDay": "in 24 hours",
        "days": "in 2 days",
        "exactDay": "in 1 day",
        "exactHour": "in 1 hour",
        "exactWeek": "in 1 week",
        "hours": "in 2 hours",
        "past": "soon",
        "soon": "soon",
        "weeks": "in 2 weeks",
      }
    `)
  })

  test("formats values for datetime-local inputs", () => {
    expect({
      milliseconds: formatDateTimeLocalInput(DATE, {timeZone: "UTC"}),
      seconds: formatDateTimeLocalInput(DATE / 1000, {timeZone: "UTC", unit: "seconds"}),
      shifted: formatDateTimeLocalInput(DATE, {timeZone: "Asia/Yerevan"}),
      midnight: formatDateTimeLocalInput("2026-08-05T00:00:00.000Z", {
        timeZone: "UTC",
      }),
      missing: formatDateTimeLocalInput(undefined, {timeZone: "UTC"}),
      invalid: formatDateTimeLocalInput("not-a-date", {timeZone: "UTC"}),
    }).toMatchInlineSnapshot(`
      {
        "invalid": "",
        "midnight": "2026-08-05T00:00:00",
        "milliseconds": "2026-08-05T10:30:15",
        "missing": "",
        "seconds": "2026-08-05T10:30:15",
        "shifted": "2026-08-05T14:30:15",
      }
    `)
  })

  test("normalizes seconds and renders semantic time elements", () => {
    expect(dateTimeValueToDate(DATE / 1000, "seconds")?.toISOString()).toBe(
      "2026-08-05T10:30:15.000Z",
    )
    expect(
      renderToStaticMarkup(
        createElement(DateTime, {
          value: DATE / 1000,
          unit: "seconds",
          display: "date-time",
          locale: "en-US",
          timeZone: "UTC",
          tooltip: false,
        }),
      ),
    ).toBe(
      `<time data-visual-dynamic="time" data-visual-placeholder="&lt;time&gt;" dateTime="2026-08-05T10:30:15.000Z">${expectedDateTime("Aug 5, 2026", "10:30")}</time>`,
    )
    expect(
      renderToStaticMarkup(
        createElement(RelativeTime, {
          value: DATE,
          now: NOW,
          locale: "en-US",
          timeZone: "UTC",
          tooltip: false,
        }),
      ),
    ).toMatchInlineSnapshot(
      `"<time data-visual-dynamic="time" data-visual-placeholder="&lt;time&gt;" dateTime="2026-08-05T10:30:15.000Z">1h ago</time>"`,
    )
  })
})

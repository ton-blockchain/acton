import {DateTime, RelativeTime} from "@acton/ui"

import styles from "./dateTimeGallery.module.css"
import type {ComponentGallery} from "./types"

const sampleDate = Date.UTC(2026, 7, 5, 10, 30, 15)
const sampleNow = Date.UTC(2026, 7, 5, 12, 0)

function FormatSamples() {
  return (
    <div className={styles.grid}>
      {[
        ["Date", <DateTime key="date" value={sampleDate} display="date" />],
        ["Numeric date", <DateTime key="date-numeric" value={sampleDate} display="date-numeric" />],
        ["Date and time", <DateTime key="date-time" value={sampleDate} />],
        [
          "Numeric date and time",
          <DateTime key="date-time-numeric" value={sampleDate} display="date-time-numeric" />,
        ],
        [
          "With seconds",
          <DateTime key="date-time-seconds" value={sampleDate} display="date-time-seconds" />,
        ],
        ["Time", <DateTime key="time" value={sampleDate} display="time" />],
      ].map(([label, value]) => (
        <div className={styles.item} key={label as string}>
          <span>{label}</span>
          <strong>{value}</strong>
        </div>
      ))}
    </div>
  )
}

function RelativeSamples() {
  return (
    <div className={styles.grid}>
      <div className={styles.item}>
        <span>Past</span>
        <strong>
          <RelativeTime value={sampleDate} now={sampleNow} />
        </strong>
      </div>
      <div className={styles.item}>
        <span>Future</span>
        <strong>
          <RelativeTime value={sampleNow + 2 * 60 * 60 * 1000} now={sampleNow} />
        </strong>
      </div>
      <div className={styles.item}>
        <span>Hybrid</span>
        <strong>
          <RelativeTime value={sampleNow - 2 * 86_400_000} now={sampleNow} mode="hybrid" />
        </strong>
      </div>
    </div>
  )
}

export const dateTimeGallery = {
  id: "date-time",
  title: "DateTime",
  status: "ready",
  summary:
    "DateTime and RelativeTime provide consistent, semantic calendar timestamps across Acton interfaces",
  importStatement:
    'import {DateTime, RelativeTime, formatDateTimeLocalInput, formatTimeUntil} from "@acton/ui"',
  agentSummary:
    "Use DateTime for absolute timestamps and RelativeTime for recent activity. Human-readable and stable numeric day-month-year presets render semantic time elements and accept seconds or milliseconds explicitly.",
  usage: [
    "Use the default date-time display for table cells and metadata",
    "Use date-time-seconds only when second-level precision is useful",
    "Use date-numeric or date-time-numeric when the interface requires a stable numeric day-month-year value",
    "Use RelativeTime for activity feeds and provide a shared now value when many rows update together",
    "Set unit to seconds for UNIX timestamps returned by TON APIs",
    "Keep the default tooltip for relative time, timezone, and copyable UNIX and ISO values",
    "Use formatTimeUntil for schedule copy and formatDateTimeLocalInput for datetime-local controls",
  ],
  avoid: [
    "Do not create local Intl.DateTimeFormat instances for standard UI timestamps",
    "Do not infer whether a number is seconds or milliseconds from its size",
    "Do not use calendar components for execution duration, latency, or gas timing",
  ],
  sections: [
    {
      id: "date-time-formats",
      title: "Absolute time",
      description: "Shared presets keep dates readable without hiding requested precision",
      content: <FormatSamples />,
    },
    {
      id: "date-time-relative",
      title: "Relative time",
      description: "Relative labels keep the full absolute timestamp available on hover",
      content: <RelativeSamples />,
    },
  ],
} satisfies ComponentGallery

import {Duration} from "@acton/ui"

import styles from "./durationGallery.module.css"
import type {ComponentGallery} from "./types"

function DurationSamples() {
  return (
    <div className={styles.grid}>
      {[
        ["Compact", <Duration key="compact" value={7320} />],
        ["Elapsed", <Duration key="elapsed" display="elapsed" value={125} />],
        ["Human", <Duration key="human" display="human" value={125} />],
        [
          "Latency",
          <Duration key="latency" display="latency" unit="nanoseconds" value={1_234_567} />,
        ],
        [
          "Runtime",
          <Duration key="runtime" display="runtime" unit="milliseconds" value={65_400} />,
        ],
        ["Precise", <Duration key="precise" display="precise" unit="milliseconds" value={0.125} />],
      ].map(([label, value]) => (
        <div className={styles.item} key={label as string}>
          <span>{label}</span>
          <strong>{value}</strong>
        </div>
      ))}
    </div>
  )
}

export const durationGallery = {
  id: "duration",
  title: "Duration",
  status: "ready",
  summary: "Duration formats elapsed time, latency, and runtime values consistently",
  importStatement:
    'import {Duration, formatDuration, formatRecurringPeriod, formatSchedulePeriod} from "@acton/ui"',
  agentSummary:
    "Use Duration for elapsed time and execution measurements, with an explicit source unit and display preset",
  usage: [
    "Use compact for coarse uptime values",
    "Use elapsed for active operations and human for trace summaries",
    "Use latency for nanosecond API measurements",
    "Use runtime or precise for test execution results",
    "Use formatDuration only when JSX cannot contain the component",
    "Use formatSchedulePeriod for exact schedule intervals",
    "Use formatRecurringPeriod for request and refresh windows",
  ],
  avoid: [
    "Do not create local duration formatters",
    "Do not infer whether a number uses nanoseconds, milliseconds, or seconds",
    "Do not use DateTime for elapsed time",
  ],
  sections: [
    {
      id: "duration-formats",
      title: "Formats",
      description: "Presets preserve useful precision for each type of measurement",
      content: <DurationSamples />,
    },
  ],
} satisfies ComponentGallery

export interface LanguageServerProfile {
  readonly enabled: boolean
  readonly counters: Readonly<Record<string, number>>
  readonly spans: Readonly<
    Record<string, {readonly count: number; readonly totalMs: number; readonly averageMs: number}>
  >
}

export function renderLanguageServerProfile(profile: LanguageServerProfile): string {
  const counters = Object.entries(profile.counters)
  const spans = Object.entries(profile.spans)
  const lines = [
    "Acton Language Server Profile",
    `Status: ${profile.enabled ? "enabled" : "disabled"}`,
  ]

  if (counters.length > 0) {
    const nameWidth = Math.max("Counter".length, ...counters.map(([name]) => name.length))
    lines.push("", "Counters", `  ${"Counter".padEnd(nameWidth)}  Count`)
    for (const [name, count] of counters) {
      lines.push(`  ${name.padEnd(nameWidth)}  ${count}`)
    }
  }

  if (spans.length > 0) {
    const nameWidth = Math.max("Span".length, ...spans.map(([name]) => name.length))
    const countWidth = Math.max(
      "Count".length,
      ...spans.map(([, span]) => String(span.count).length),
    )
    const rows = spans.map(([name, span]) => ({
      name,
      count: String(span.count),
      total: formatMilliseconds(span.totalMs),
      average: formatMilliseconds(span.averageMs),
    }))
    const totalWidth = Math.max("Total".length, ...rows.map(row => row.total.length))
    const averageWidth = Math.max("Average".length, ...rows.map(row => row.average.length))
    lines.push(
      "",
      "Spans",
      `  ${"Span".padEnd(nameWidth)}  ${"Count".padStart(countWidth)}  ` +
        `${"Total".padStart(totalWidth)}  ${"Average".padStart(averageWidth)}`,
    )
    for (const row of rows) {
      lines.push(
        `  ${row.name.padEnd(nameWidth)}  ${row.count.padStart(countWidth)}  ` +
          `${row.total.padStart(totalWidth)}  ${row.average.padStart(averageWidth)}`,
      )
    }
  }

  if (counters.length === 0 && spans.length === 0) {
    lines.push(
      "",
      profile.enabled ? "No profiling data." : "Enable profiling and restart the server.",
    )
  }

  return `${lines.join("\n")}\n`
}

function formatMilliseconds(value: number): string {
  return `${value.toFixed(3)}ms`
}

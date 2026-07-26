import type {TestRunStatus, TestRunSummary} from "./studioApi"

export function testRunStatusLabel(status: TestRunStatus) {
  const labels: Record<TestRunStatus, string> = {
    queued: "Queued",
    running: "Running",
    passed: "Passed",
    failed: "Failed",
    cancelled: "Cancelled",
  }
  return labels[status]
}

export function testRunLabel(run: TestRunSummary) {
  if (run.status === "running") return "Running tests"
  if (run.stats.total === 1) return "1 test"
  return `${run.stats.total} tests`
}

export function testRunSummary(run: TestRunSummary) {
  if (run.status === "running" || run.status === "queued") return testRunStatusLabel(run.status)
  if (run.status === "cancelled") return "Cancelled"
  if (run.stats.total === 0 && run.status === "failed") return "Failed before tests started"
  return `${run.stats.passed} passed${run.stats.failed ? `, ${run.stats.failed} failed` : ""}`
}

export function testRunTime(value: string) {
  const date = new Date(value)
  const today = new Date()
  const sameDay = date.toDateString() === today.toDateString()
  return new Intl.DateTimeFormat(undefined, {
    ...(sameDay ? {} : {month: "short", day: "numeric"}),
    hour: "2-digit",
    minute: "2-digit",
  }).format(date)
}

export function formatTestRunDuration(durationMs: number) {
  if (durationMs < 1000) return `${durationMs} ms`
  if (durationMs < 60_000) return `${(durationMs / 1000).toFixed(1)} s`
  return `${Math.floor(durationMs / 60_000)}m ${Math.round((durationMs % 60_000) / 1000)}s`
}

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

import {
  Button,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
} from "@acton/ui"
import {FlaskConical} from "lucide-react"

import type {TestRunStatus, TestRunSummary} from "../studioApi"
import {
  formatTestRunDuration,
  testRunLabel,
  testRunStatusLabel,
  testRunSummary,
  testRunTime,
} from "../testRunPresentation"

import styles from "./TestRunsTable.module.css"

interface TestRunsTableProps {
  readonly isLoading: boolean
  readonly runs: readonly TestRunSummary[]
  readonly onOpenRun: (runId: string) => void
  readonly onRunTests: () => void
}

export function TestRunsTable({isLoading, runs, onOpenRun, onRunTests}: TestRunsTableProps) {
  return (
    <DataTable minWidth="48rem">
      <DataTableTable aria-label="Test runs">
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell columnWidth="24%">Run</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="14%">Status</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="12%">Source</DataTableHeaderCell>
            <DataTableHeaderCell>Result</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="11%">Duration</DataTableHeaderCell>
            <DataTableHeaderCell align="right" columnWidth="9rem">
              Started
            </DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {isLoading ? (
            <DataTableSkeletonRows
              columns={6}
              rows={4}
              widths={["54%", "48%", "42%", "68%", "45%", "60%"]}
              alignments={["left", "left", "left", "left", "left", "right"]}
            />
          ) : runs.length === 0 ? (
            <DataTableEmpty colSpan={6}>
              <div className={styles.emptyState}>
                <span className={styles.emptyIcon}>
                  <FlaskConical size={21} aria-hidden="true" />
                </span>
                <strong>No test runs</strong>
                <span>Run tests from Studio or use acton test in the terminal</span>
                <Button
                  size="sm"
                  variant="primary"
                  leadingIcon={<FlaskConical size={15} aria-hidden="true" />}
                  onClick={onRunTests}
                >
                  Run tests
                </Button>
              </div>
            </DataTableEmpty>
          ) : (
            runs.map(run => (
              <DataTableRow
                key={run.id}
                hover
                interactive
                onClick={event => {
                  const target = event.target
                  if (target instanceof Element && target.closest("button, a")) return
                  onOpenRun(run.id)
                }}
              >
                <DataTableCell>
                  <div className={styles.runName}>
                    <button type="button" onClick={() => onOpenRun(run.id)}>
                      {testRunLabel(run)}
                    </button>
                    {run.error ? <span title={run.error}>{run.error}</span> : null}
                  </div>
                </DataTableCell>
                <DataTableCell>
                  <RunStatus status={run.status} />
                </DataTableCell>
                <DataTableCell tone="muted">
                  {run.source === "studio" ? "Studio" : "CLI"}
                </DataTableCell>
                <DataTableCell tone="muted">{testRunSummary(run)}</DataTableCell>
                <DataTableCell tone="muted">
                  {formatTestRunDuration(run.stats.durationMs)}
                </DataTableCell>
                <DataTableCell align="right" tone="muted">
                  <time dateTime={run.startedAt} title={formatStartedAt(run.startedAt)}>
                    {testRunTime(run.startedAt)}
                  </time>
                </DataTableCell>
              </DataTableRow>
            ))
          )}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

function RunStatus({status}: {readonly status: TestRunStatus}) {
  return (
    <span className={styles.status} data-status={status}>
      <span className={styles.statusDot} aria-hidden="true" />
      {testRunStatusLabel(status)}
    </span>
  )
}

function formatStartedAt(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "medium",
  }).format(new Date(value))
}

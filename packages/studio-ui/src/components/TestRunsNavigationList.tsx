import {DateTime} from "@acton/ui"
import {Ban, Check, LoaderCircle, X} from "lucide-react"

import type {TestRunStatus, TestRunSummary} from "../studioApi"
import {testRunLabel, testRunSummary} from "../testRunPresentation"

import styles from "./StudioNavigation.module.css"

interface TestRunsNavigationListProps {
  readonly runs: readonly TestRunSummary[]
  readonly selectedRunId?: string
  readonly onSelect: (runId: string) => void
}

export function TestRunsNavigationList({
  runs,
  selectedRunId,
  onSelect,
}: TestRunsNavigationListProps) {
  return (
    <ul className={styles.testRunNavList} aria-label="Test runs">
      {runs.map(run => (
        <li key={run.id}>
          <button
            type="button"
            className={`${styles.testRunNavItem} ${
              selectedRunId === run.id ? styles.testRunNavItemActive : ""
            }`}
            aria-current={selectedRunId === run.id ? "page" : undefined}
            onClick={() => onSelect(run.id)}
          >
            <TestRunNavigationStatus status={run.status} />
            <span className={styles.testRunNavBody}>
              <span className={styles.testRunNavTop}>
                <strong>{testRunLabel(run)}</strong>
                <DateTime value={run.startedAt} display="smart" />
              </span>
              <span className={styles.testRunNavMeta}>
                {run.source === "studio" ? "Studio" : "CLI"}
                <span aria-hidden="true">·</span>
                {testRunSummary(run)}
              </span>
            </span>
          </button>
        </li>
      ))}
    </ul>
  )
}

function TestRunNavigationStatus({status}: {readonly status: TestRunStatus}) {
  if (status === "running" || status === "queued") {
    return (
      <LoaderCircle
        className={styles.testRunNavSpinner}
        data-run-status={status}
        aria-hidden="true"
      />
    )
  }
  if (status === "passed") return <Check data-run-status={status} aria-hidden="true" />
  if (status === "failed") return <X data-run-status={status} aria-hidden="true" />
  return <Ban data-run-status={status} aria-hidden="true" />
}

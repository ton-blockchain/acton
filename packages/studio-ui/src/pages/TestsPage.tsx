import type {TestReport} from "@acton/test-ui/embed"
import {Button, Dialog, Duration} from "@acton/ui"
import {
  Ban,
  Check,
  CircleAlert,
  CircleDot,
  FileTerminal,
  FlaskConical,
  LoaderCircle,
  Square,
  X,
} from "lucide-react"
import {lazy, Suspense, useEffect, useMemo, useState} from "react"

import {TablePage} from "../components/TablePage"
import {TestRunsTable} from "../components/TestRunsTable"
import type {StudioTestRunsState} from "../hooks/useStudioTestRuns"
import {
  type TestRunOutput,
  type TestRunRecord,
  type TestRunStatus,
  studioTestRunArtifactsUrl,
} from "../studioApi"
import {testRunStatusLabel} from "../testRunPresentation"
import {RunTestsDialog} from "./RunTestsDialog"

import styles from "./TestsPage.module.css"

const EmbeddedTestDetails = lazy(() => import("./EmbeddedTestDetails"))

interface TestsPageProps {
  readonly runDialogOpen: boolean
  readonly selectedTestKey?: string
  readonly testRuns: StudioTestRunsState
  readonly onSelectedTestKeyChange: (testKey: string, replace: boolean) => void
  readonly onRunDialogOpenChange: (open: boolean) => void
}

export function TestsPage({
  runDialogOpen,
  selectedTestKey,
  testRuns,
  onSelectedTestKeyChange,
  onRunDialogOpenChange,
}: TestsPageProps) {
  const [outputDialogOpen, setOutputDialogOpen] = useState(false)
  const {runs, selectedRunId, selectedRun, output, isLoading, isCancelling, error} = testRuns

  if (!selectedRunId) {
    return (
      <>
        <TablePage
          error={error}
          errorTitle="Unable to load test runs"
          hasContent={runs.length > 0}
          onRetry={testRuns.refresh}
        >
          <TestRunsTable
            isLoading={isLoading}
            runs={runs}
            onOpenRun={testRuns.selectRun}
            onRunTests={() => onRunDialogOpenChange(true)}
          />
        </TablePage>
        <RunTestsDialog
          open={runDialogOpen}
          onOpenChange={onRunDialogOpenChange}
          onStarted={testRuns.setStartedRun}
        />
      </>
    )
  }

  return (
    <div className={styles.page}>
      {error ? (
        <section className={styles.errorPanel} aria-live="polite">
          <CircleAlert size={18} aria-hidden="true" />
          <span>{error}</span>
          <Button size="sm" variant="outline" onClick={() => void testRuns.refresh()}>
            Retry
          </Button>
        </section>
      ) : null}

      <div className={styles.workspace}>
        <section className={styles.runPanel}>
          <header className={styles.runHeader}>
            {selectedRun ? (
              <>
                <div className={styles.runTitle}>
                  <RunStatusIcon status={selectedRun.status} />
                  <strong>{testRunStatusLabel(selectedRun.status)}</strong>
                </div>
                <RunStats run={selectedRun} />
              </>
            ) : (
              <div className={styles.runTitle}>
                <LoaderCircle className={styles.spinner} size={16} aria-hidden="true" />
                <strong>Loading run</strong>
              </div>
            )}
            <div className={styles.runActions}>
              {selectedRun ? (
                <Button
                  size="sm"
                  variant="secondary"
                  leadingIcon={<FileTerminal size={14} aria-hidden="true" />}
                  onClick={() => setOutputDialogOpen(true)}
                >
                  Runner output
                </Button>
              ) : null}
              {selectedRun?.source === "studio" && selectedRun.status === "running" ? (
                <Button
                  size="sm"
                  variant="danger"
                  loading={isCancelling}
                  leadingIcon={<Square size={13} aria-hidden="true" />}
                  onClick={() => void testRuns.cancelSelectedRun()}
                >
                  Stop run
                </Button>
              ) : null}
              <Button
                size="sm"
                variant="primary"
                leadingIcon={<FlaskConical size={14} aria-hidden="true" />}
                onClick={() => onRunDialogOpenChange(true)}
              >
                Run tests
              </Button>
            </div>
          </header>
          {selectedRun ? (
            <>
              {selectedRun.error ? (
                <div className={styles.runError} role="alert">
                  {selectedRun.error}
                </div>
              ) : null}
              <TestResults
                run={selectedRun}
                selectedKey={selectedTestKey}
                onSelectedKeyChange={onSelectedTestKeyChange}
              />
            </>
          ) : (
            <div className={styles.runEmpty}>
              <LoaderCircle className={styles.spinner} size={20} aria-hidden="true" />
              <strong>Loading test run</strong>
            </div>
          )}
        </section>
      </div>

      <RunTestsDialog
        open={runDialogOpen}
        onOpenChange={onRunDialogOpenChange}
        onStarted={testRuns.setStartedRun}
      />
      <Dialog
        open={outputDialogOpen && selectedRun !== undefined}
        onOpenChange={setOutputDialogOpen}
        title="Runner output"
        description="Captured output from this test run"
        maxWidth="min(52rem, calc(100vw - 32px))"
        contentClassName={styles.outputDialogContent}
      >
        {selectedRun ? <RunOutput output={output} status={selectedRun.status} /> : null}
      </Dialog>
    </div>
  )
}

function TestResults({
  run,
  selectedKey,
  onSelectedKeyChange,
}: {
  readonly run: TestRunRecord
  readonly selectedKey?: string
  readonly onSelectedKeyChange: (testKey: string, replace: boolean) => void
}) {
  const reports = run.reports
  const selectedTest =
    reports.find(report => reportKey(report) === selectedKey) ??
    reports.find(report => report.status === "Failed") ??
    reports[0]
  const suites = useMemo(() => groupReports(reports), [reports])

  useEffect(() => {
    if (!selectedTest) return
    const nextSelectedKey = reportKey(selectedTest)
    if (nextSelectedKey !== selectedKey) onSelectedKeyChange(nextSelectedKey, true)
  }, [onSelectedKeyChange, selectedKey, selectedTest])

  if (reports.length === 0) {
    return (
      <div className={styles.waitingState}>
        {run.status === "running" ? (
          <LoaderCircle className={styles.spinner} size={20} aria-hidden="true" />
        ) : (
          <CircleDot size={20} aria-hidden="true" />
        )}
        <strong>{run.status === "running" ? "Waiting for test results" : "No test results"}</strong>
        <span>Build and runner output is available from Runner output</span>
      </div>
    )
  }

  return (
    <div className={styles.results}>
      <aside className={styles.testList} aria-label="Tests in this run">
        {suites.map(([suite, suiteReports]) => (
          <section key={suite} className={styles.testSuite}>
            <header>
              <span>{suite}</span>
              <span>{suiteReports.length}</span>
            </header>
            {suiteReports.map(report => (
              <button
                key={reportKey(report)}
                type="button"
                className={styles.testItem}
                data-selected={reportKey(report) === reportKey(selectedTest) || undefined}
                onClick={() => onSelectedKeyChange(reportKey(report), false)}
              >
                <TestStatusIcon status={report.status} />
                <span>{report.name}</span>
              </button>
            ))}
          </section>
        ))}
      </aside>
      <div className={styles.testDetails}>
        <Suspense
          fallback={
            <div className={styles.waitingState}>
              <LoaderCircle className={styles.spinner} size={20} aria-hidden="true" />
              <strong>Loading test details</strong>
            </div>
          }
        >
          <EmbeddedTestDetails
            key={`${run.id}:${reportKey(selectedTest)}`}
            baseUrl={studioTestRunArtifactsUrl(run.id)}
            projectRoot={run.projectRoot}
            test={selectedTest}
          />
        </Suspense>
      </div>
    </div>
  )
}

function RunStats({run}: {readonly run: TestRunRecord}) {
  const stats = [
    ["Total", run.stats.total],
    ["Passed", run.stats.passed],
    ["Failed", run.stats.failed],
    ["Skipped", run.stats.skipped + run.stats.todo],
  ] as const

  return (
    <div className={styles.stats}>
      {stats.map(([label, value]) => (
        <span key={label}>
          <strong>{value}</strong>
          {label}
        </span>
      ))}
      <span>
        <strong>
          <Duration display="runtime" unit="milliseconds" value={run.stats.durationMs} />
        </strong>
        Duration
      </span>
    </div>
  )
}

function RunOutput({
  output,
  status,
}: {
  readonly output: TestRunOutput
  readonly status: TestRunStatus
}) {
  if (!output.stdout && !output.stderr) {
    return (
      <div className={styles.waitingState}>
        {status === "running" ? (
          <LoaderCircle className={styles.spinner} size={20} aria-hidden="true" />
        ) : (
          <CircleDot size={20} aria-hidden="true" />
        )}
        <strong>{status === "running" ? "Waiting for runner output" : "No captured output"}</strong>
        <span>Manual CLI runs keep per-test logs but do not redirect terminal output</span>
      </div>
    )
  }

  return (
    <div className={styles.output}>
      {output.stdout ? (
        <section>
          <header>stdout</header>
          <pre>{output.stdout}</pre>
        </section>
      ) : null}
      {output.stderr ? (
        <section>
          <header>stderr</header>
          <pre>{output.stderr}</pre>
        </section>
      ) : null}
    </div>
  )
}

function RunStatusIcon({status}: {readonly status: TestRunStatus}) {
  if (status === "running" || status === "queued") {
    return <LoaderCircle className={styles.spinner} data-status={status} aria-hidden="true" />
  }
  if (status === "passed") return <Check data-status={status} aria-hidden="true" />
  if (status === "failed") return <X data-status={status} aria-hidden="true" />
  return <Ban data-status={status} aria-hidden="true" />
}

function TestStatusIcon({status}: {readonly status: TestReport["status"]}) {
  if (status === "Passed") return <Check data-test-status="passed" aria-hidden="true" />
  if (status === "Failed") return <X data-test-status="failed" aria-hidden="true" />
  if (status === "Todo") return <CircleDot data-test-status="todo" aria-hidden="true" />
  return <Ban data-test-status="skipped" aria-hidden="true" />
}

function reportKey(report: Pick<TestReport, "file_path" | "name" | "row" | "column">) {
  return `${report.file_path}:${report.row}:${report.column}:${report.name}`
}

function groupReports(reports: readonly TestReport[]) {
  const suites = new Map<string, TestReport[]>()
  for (const report of reports) {
    const current = suites.get(report.suite_name) ?? []
    current.push(report)
    suites.set(report.suite_name, current)
  }
  return [...suites.entries()]
}

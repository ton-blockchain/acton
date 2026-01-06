import type React from "react"
import { type TestReport, TestStatus } from "../../types"
import { Summary } from "../Summary/Summary"
import styles from "./Sidebar.module.css"

interface SidebarProps {
  readonly reports: TestReport[]
  readonly selectedTest: TestReport | null
  readonly onSelectTest: (test: TestReport) => void
}

export const Sidebar: React.FC<SidebarProps> = ({ reports, selectedTest, onSelectTest }) => {
  const suites = reports.reduce(
    (acc, report) => {
      if (!acc[report.suite_name]) {
        acc[report.suite_name] = []
      }
      acc[report.suite_name].push(report)
      return acc
    },
    {} as Record<string, TestReport[]>,
  )

  const getStatusIcon = (status: TestStatus) => {
    switch (status) {
      case TestStatus.Passed:
        return <span className={styles.passed}>✓</span>
      case TestStatus.Failed:
        return <span className={styles.failed}>✕</span>
      case TestStatus.Skipped:
        return <span className={styles.skipped}>○</span>
      case TestStatus.Todo:
        return <span className={styles.todo}>-</span>
      default:
        return null
    }
  }

  return (
    <div className={styles.sidebar}>
      <div className={styles.header}>
        <div className={styles.title}>Acton Tests</div>
        <Summary reports={reports} />
      </div>
      <div className={styles.content}>
        {Object.entries(suites).map(([suiteName, suiteReports]) => (
          <div key={suiteName} className={styles.suite}>
            <div className={styles.suiteHeader}>{suiteName}</div>
            <div className={styles.testList}>
              {suiteReports.map((report, idx) => {
                const isSelected = selectedTest === report
                return (
                  <button
                    key={`${report.name}-${idx}`}
                    type="button"
                    className={`${styles.testItem} ${isSelected ? styles.selected : ""}`}
                    onClick={() => onSelectTest(report)}
                  >
                    {getStatusIcon(report.status)}
                    <span className={styles.testName}>{report.name}</span>
                  </button>
                )
              })}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}

import type React from "react"
import { type TestReport, TestStatus } from "../../types"
import styles from "./TestList.module.css"

interface TestListProps {
  readonly reports: TestReport[]
  readonly onViewTrace: (path: string) => void
}

export const TestList: React.FC<TestListProps> = ({ reports, onViewTrace }) => {
  const getStatusClass = (status: TestStatus) => {
    switch (status) {
      case TestStatus.Passed:
        return styles.statusPassed
      case TestStatus.Failed:
        return styles.statusFailed
      case TestStatus.Skipped:
        return styles.statusSkipped
      case TestStatus.Todo:
        return styles.statusTodo
      default:
        return ""
    }
  }

  return (
    <div className={styles.list}>
      {reports.map((report, idx) => (
        <div key={`${report.suite_name}-${report.name}-${idx}`} className={styles.item}>
          <div className={styles.info}>
            <div className={styles.suite}>{report.suite_name}</div>
            <div className={styles.name}>{report.name}</div>
            {report.message && <div className={styles.errorMessage}>{report.message}</div>}
            {report.trace_path && (
              <div className={styles.actions}>
                <button
                  type="button"
                  className={styles.actionLink}
                  onClick={() => report.trace_path && onViewTrace(report.trace_path)}
                >
                  View Trace
                </button>
              </div>
            )}
          </div>
          <div className={`${styles.status} ${getStatusClass(report.status)}`}>{report.status}</div>
        </div>
      ))}
    </div>
  )
}

import type React from "react"
import { type TestReport, TestStatus } from "@acton/ui-shared"
import styles from "./Summary.module.css"

interface SummaryProps {
  readonly reports: TestReport[]
}

export const Summary: React.FC<SummaryProps> = ({ reports }) => {
  const total = reports.length
  const passed = reports.filter((r) => r.status === TestStatus.Passed).length
  const failed = reports.filter((r) => r.status === TestStatus.Failed).length

  return (
    <div className={styles.summary}>
      <div className={styles.card}>
        <span className={styles.count}>{total}</span>
        <span className={styles.label}>Total</span>
      </div>
      <div className={styles.card}>
        <span className={`${styles.count} ${styles.passed}`}>{passed}</span>
        <span className={styles.label}>Passed</span>
      </div>
      <div className={styles.card}>
        <span className={`${styles.count} ${styles.failed}`}>{failed}</span>
        <span className={styles.label}>Failed</span>
      </div>
    </div>
  )
}

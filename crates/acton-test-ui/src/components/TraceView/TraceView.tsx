import type React from "react"
import type { Trace } from "../../types"
import styles from "./TraceView.module.css"

interface TraceViewProps {
  readonly trace: Trace
  readonly onBack: () => void
}

export const TraceView: React.FC<TraceViewProps> = ({ trace, onBack }) => {
  return (
    <div className={styles.container}>
      <div className={styles.header}>
        <h1>Trace: {trace.name}</h1>
        <button type="button" onClick={onBack} className={styles.backLink}>
          ← Back to Tests
        </button>
      </div>

      {trace.txs.transactions.map((tx, i) => (
        <div key={`${tx.dest_contract_info}-${i}`} className={styles.txCard}>
          <div className={styles.txHeader}>
            <div className={styles.txTitle}>
              Transaction #{i + 1}
              <span className={`${styles.tag} ${styles.tagDest}`}>
                {i === 0 ? "Trigger" : "Internal"}
              </span>
            </div>
            <div className={styles.txMeta}>
              Dest: <span className={styles.contractName}>{tx.dest_contract_info}</span>
            </div>
          </div>

          <div className={styles.logSection}>
            <div className={styles.logTitle}>VM Log Diff</div>
            <pre className={styles.pre}>{tx.vm_log_diff}</pre>
          </div>

          <div className={styles.logSection}>
            <div className={styles.logTitle}>Executor Logs</div>
            <pre className={styles.pre}>{tx.executor_logs}</pre>
          </div>
        </div>
      ))}
    </div>
  )
}

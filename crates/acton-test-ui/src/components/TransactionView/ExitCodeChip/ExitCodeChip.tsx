import type React from "react"
import styles from "./ExitCodeViewer.module.css"
import { EXIT_CODE_DESCRIPTIONS } from "./error-codes"

interface ExitCodeChipProps {
  readonly exitCode: number
  readonly exitCodes?: Record<number, string>
  readonly onOpenFile?: (uri: string, row: number, column: number) => void
}

export const ExitCodeChip: React.FC<ExitCodeChipProps> = ({ exitCode, exitCodes }) => {
  const isSuccess = exitCode === 0 || exitCode === 1
  const standardDesc = (
    EXIT_CODE_DESCRIPTIONS as Record<number, { name: string; description: string; phase: string }>
  )[exitCode]
  const customDesc = exitCodes?.[exitCode]

  return (
    <div className={`${styles.chip} ${isSuccess ? styles.success : styles.error}`}>
      <span className={styles.code}>{exitCode}</span>
      {(standardDesc || customDesc) && (
        <span className={styles.description}>{customDesc || standardDesc.name}</span>
      )}
    </div>
  )
}

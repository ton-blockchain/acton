import type { Abi } from "@/types"
import { Tooltip } from "@/index"
import styles from "./ExitCodeViewer.module.css"
import { EXIT_CODE_DESCRIPTIONS } from "./error-codes"

interface ExitCodeViewerProps {
  readonly exitCode: number | undefined
  readonly abi?: Abi | null
}

export function ExitCodeChip({ exitCode, abi }: ExitCodeViewerProps) {
  if (exitCode === undefined) {
    return <span className={styles.exitCode}>—</span>
  }

  const standardDescription = (
    EXIT_CODE_DESCRIPTIONS as Record<number, { name: string; description: string; phase: string }>
  )[exitCode] ?? {
    name: "Custom error",
    description: "User defined error",
    phase: "Compute phase",
  }

  const customErrorDescription = abi?.exitCodes?.[exitCode]

  const displayName = standardDescription?.name ?? (customErrorDescription ? "Custom error" : "")
  const description = customErrorDescription ?? standardDescription?.description
  const phase = standardDescription?.phase

  const tooltipContent = (
    <div className={styles.tooltipContent}>
      {description && (
        <div className={styles.tooltipSection}>
          <div className={styles.tooltipLabel}>Description:</div>
          <div className={styles.tooltipDescription}>{description}</div>
        </div>
      )}
      {phase && (
        <div className={styles.tooltipSection}>
          <div className={styles.tooltipLabel}>Origin:</div>
          <div className={styles.tooltipPhase}>{phase}</div>
        </div>
      )}
    </div>
  )

  const isSuccess = exitCode === 0 || exitCode === 1
  const className = `${styles.exitCode} ${isSuccess ? styles.success : styles.error}`

  return (
    <Tooltip content={tooltipContent} variant="hover">
      <span className={className}>
        {exitCode}
        {displayName && exitCode !== 0 && (
          <span className={styles.exitCodeName}> ({displayName})</span>
        )}
      </span>
    </Tooltip>
  )
}

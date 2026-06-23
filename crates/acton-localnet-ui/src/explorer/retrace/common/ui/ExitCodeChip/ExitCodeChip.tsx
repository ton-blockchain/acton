import type {ContractABI} from "@ton/core"

import {Tooltip} from "@retrace/ui/Tooltip"

import {EXIT_CODE_DESCRIPTIONS} from "@retrace/common/lib/error-codes/error-codes"

import styles from "./ExitCodeViewer.module.css"

interface ExitCodeViewerProps {
  readonly exitCode: number | undefined
  readonly abi?: ContractABI | null
}

export function ExitCodeChip({exitCode, abi}: ExitCodeViewerProps) {
  if (exitCode === undefined) {
    return <span className={styles.exitCode}>—</span>
  }

  const standardDescription = EXIT_CODE_DESCRIPTIONS[
    exitCode as keyof typeof EXIT_CODE_DESCRIPTIONS
  ] ?? {
    name: "Custom error",
    description: "User defined error",
    phase: "Compute phase",
  }
  const abiError = abi?.errors?.[exitCode]
  const customError = abiError && !standardDescription

  const displayName = standardDescription?.name ?? (customError ? "Custom error" : "")
  const description = abiError?.message ?? standardDescription?.description
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

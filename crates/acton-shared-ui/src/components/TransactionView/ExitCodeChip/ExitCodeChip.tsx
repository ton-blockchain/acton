import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import {Tooltip} from "@/index"

import styles from "./ExitCodeViewer.module.css"
import {EXIT_CODE_DESCRIPTIONS, type ExitCodeDescription, getExitCodeDocsUrl} from "./error-codes"

interface ExitCodeViewerProps {
  readonly exitCode: number | undefined
  readonly abi?: ContractABI | undefined
  readonly phase?: "compute" | "action"
}

interface CustomExitCodeInfo {
  readonly symbolicName: string
  readonly description: string
}

interface FallbackExitCodeInfo {
  readonly name: string
  readonly description: string
  readonly origin: string
}

const getCustomExitCodeInfo = (
  exitCode: number,
  abi: ContractABI | undefined,
): CustomExitCodeInfo | undefined => {
  const thrownError = abi?.thrown_errors?.find(error => error.err_code === exitCode)
  const symbolicName = thrownError?.name

  if (!symbolicName) {
    return
  }

  return {
    symbolicName,
    description: thrownError.description ?? symbolicName,
  }
}

const getFallbackExitCodeInfo = (phase: "compute" | "action"): FallbackExitCodeInfo => ({
  name: "Custom Exit Code",
  description:
    "Contract returned a user-defined exit code that is not declared in the ABI, so no symbolic description is available for this value.",
  origin: phase === "action" ? "Action phase" : "Compute phase",
})

export function ExitCodeChip({exitCode, abi, phase = "compute"}: ExitCodeViewerProps) {
  if (exitCode === undefined) {
    return <span className={styles.exitCode}>—</span>
  }

  const standardDescription = (EXIT_CODE_DESCRIPTIONS as Record<number, ExitCodeDescription>)[
    exitCode
  ]
  const customExitCode = getCustomExitCodeInfo(exitCode, abi)
  const fallbackExitCode =
    standardDescription || customExitCode ? undefined : getFallbackExitCodeInfo(phase)
  const displayName =
    standardDescription?.name ?? customExitCode?.symbolicName ?? fallbackExitCode?.name ?? ""
  const description =
    standardDescription?.description ?? customExitCode?.description ?? fallbackExitCode?.description
  const origin =
    standardDescription?.phase ??
    (customExitCode
      ? phase === "action"
        ? "Action phase"
        : "Compute phase"
      : fallbackExitCode?.origin)
  const docsUrl = standardDescription ? getExitCodeDocsUrl(exitCode) : undefined

  const tooltipContent = (
    <div className={styles.tooltipContent}>
      {description && (
        <div className={styles.tooltipSection}>
          <div className={styles.tooltipLabel}>Description:</div>
          <div className={styles.tooltipDescription}>{description}</div>
          {docsUrl && (
            <div className={styles.tooltipDocs}>
              Learn more in{" "}
              <a href={docsUrl} target="_blank" rel="noreferrer" className={styles.tooltipLink}>
                documentation
              </a>
            </div>
          )}
        </div>
      )}
      {origin && (
        <div className={styles.tooltipSection}>
          <div className={styles.tooltipLabel}>Origin:</div>
          <div className={styles.tooltipPhase}>{origin}</div>
        </div>
      )}
      {customExitCode && customExitCode.symbolicName !== description && (
        <div className={styles.tooltipSection}>
          <div className={styles.tooltipLabel}>Error:</div>
          <div className={styles.tooltipDescription}>{customExitCode.symbolicName}</div>
        </div>
      )}
    </div>
  )

  const isSuccess = phase === "action" ? exitCode === 0 : exitCode === 0 || exitCode === 1
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

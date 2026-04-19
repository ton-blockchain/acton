import type {Abi, CompilerAbi} from "@/types"
import {Tooltip} from "@/index"

import styles from "./ExitCodeViewer.module.css"
import {EXIT_CODE_DESCRIPTIONS, getExitCodeDocsUrl, type ExitCodeDescription} from "./error-codes"

interface ExitCodeViewerProps {
  readonly exitCode: number | undefined
  readonly abi?: Abi | undefined
  readonly compilerAbi?: CompilerAbi | undefined
  readonly phase?: "compute" | "action"
}

interface CustomExitCodeInfo {
  readonly symbolicName: string
  readonly description: string
}

const getCompilerAbiSymbolDescription = (
  compilerAbi: CompilerAbi | undefined,
  symbol: string,
): string | undefined => {
  const enumMatch = symbol.split(".")
  if (enumMatch.length >= 2) {
    const memberName = enumMatch.at(-1)
    const enumName = enumMatch.at(-2)
    const enumDeclaration = compilerAbi?.declarations?.find(
      declaration => declaration.kind === "enum" && declaration.name === enumName,
    )
    const memberDescription = enumDeclaration?.members?.find(
      member => member.name === memberName && member.description,
    )?.description
    if (memberDescription) {
      return memberDescription
    }
  }

  return compilerAbi?.constants?.find(constant => constant.name === symbol && constant.description)
    ?.description
}

const getCustomExitCodeInfo = (
  exitCode: number,
  abi: Abi | undefined,
  compilerAbi: CompilerAbi | undefined,
): CustomExitCodeInfo | undefined => {
  const thrownError = compilerAbi?.thrown_errors?.find(error => error.err_code === exitCode)
  const symbolicName =
    thrownError?.name ||
    abi?.exitCodes?.find(candidate => candidate.value === exitCode)?.constantName

  if (!symbolicName) {
    return
  }

  return {
    symbolicName,
    description: getCompilerAbiSymbolDescription(compilerAbi, symbolicName) ?? symbolicName,
  }
}

export function ExitCodeChip({exitCode, abi, compilerAbi, phase = "compute"}: ExitCodeViewerProps) {
  if (exitCode === undefined) {
    return <span className={styles.exitCode}>—</span>
  }

  const standardDescription = (EXIT_CODE_DESCRIPTIONS as Record<number, ExitCodeDescription>)[
    exitCode
  ]
  const customExitCode = getCustomExitCodeInfo(exitCode, abi, compilerAbi)
  const displayName = standardDescription?.name ?? customExitCode?.symbolicName ?? ""
  const description = standardDescription?.description ?? customExitCode?.description
  const origin =
    standardDescription?.phase ??
    (customExitCode ? (phase === "action" ? "Action phase" : "Compute phase") : undefined)
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

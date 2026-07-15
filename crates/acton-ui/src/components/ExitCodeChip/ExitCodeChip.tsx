import {cx} from "../../lib/cx"
import {Popover} from "../Popover"
import styles from "./ExitCodeChip.module.css"
import {type ExitCodeAbi, type ExitCodePhase, resolveExitCode} from "./error-codes"

export type {ExitCodeAbi, ExitCodePhase} from "./error-codes"

export interface ExitCodeChipProps {
  readonly exitCode: number | undefined
  readonly abi?: ExitCodeAbi
  readonly phase?: ExitCodePhase
}

export function ExitCodeChip({exitCode, abi, phase = "compute"}: ExitCodeChipProps) {
  if (exitCode === undefined) {
    return <span className={cx(styles.exitCode, styles.unknown)}>Unknown</span>
  }

  const {customSymbolicName, description, displayName, docsUrl, isSuccess, origin} =
    resolveExitCode(exitCode, abi, phase)

  const popoverContent = (
    <div className={styles.popoverContent}>
      <div className={styles.popoverSection}>
        <div className={styles.popoverLabel}>Description</div>
        <div>{description}</div>
        {docsUrl ? (
          <div className={styles.popoverDocs}>
            Learn more in{" "}
            <a href={docsUrl} target="_blank" rel="noreferrer" className={styles.popoverLink}>
              documentation
            </a>
          </div>
        ) : undefined}
      </div>
      <div className={styles.popoverSection}>
        <div className={styles.popoverLabel}>Origin</div>
        <div>{origin}</div>
      </div>
      {customSymbolicName && customSymbolicName !== description ? (
        <div className={styles.popoverSection}>
          <div className={styles.popoverLabel}>Error</div>
          <div>{customSymbolicName}</div>
        </div>
      ) : undefined}
    </div>
  )

  return (
    <Popover
      content={popoverContent}
      placement="top"
      ariaLabel={`Exit code ${exitCode}: ${displayName}`}
    >
      <span
        className={cx(
          styles.exitCode,
          styles.interactive,
          isSuccess ? styles.success : styles.error,
        )}
      >
        {exitCode}
        {exitCode === 0 ? undefined : <span className={styles.exitCodeName}> ({displayName})</span>}
      </span>
    </Popover>
  )
}

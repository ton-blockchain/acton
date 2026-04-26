import type React from "react"

import {Tooltip} from "@/components/Tooltip/Tooltip"
import {parseChangeLibraryMode} from "@/components/TransactionView/ChangeLibraryModeViewer/parser"

import styles from "./ChangeLibraryModeViewer.module.css"

interface ChangeLibraryModeViewerProps {
  readonly mode: number | undefined
}

export const ChangeLibraryModeViewer: React.FC<ChangeLibraryModeViewerProps> = ({mode}) => {
  if (mode === undefined) {
    return <span className={styles.empty}>No mode</span>
  }

  const flags = parseChangeLibraryMode(mode)

  return (
    <div className={styles.container}>
      {flags.map((flag, index) => (
        <span key={`${flag.name}-${flag.value}`} className={styles.modeItem}>
          {index > 0 && <span className={styles.plus}> + </span>}
          <Tooltip
            content={<div className={styles.tooltipDescription}>{flag.description}</div>}
            variant="hover"
          >
            <span className={styles.constant}>
              {flag.name} ({flag.value})
            </span>
          </Tooltip>
        </span>
      ))}
    </div>
  )
}

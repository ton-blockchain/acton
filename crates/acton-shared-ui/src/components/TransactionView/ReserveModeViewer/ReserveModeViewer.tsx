import type React from "react"

import {Tooltip} from "@/components/Tooltip/Tooltip"
import {parseReserveMode} from "@/utils/transaction"

import styles from "./ReserveModeViewer.module.css"

interface ReserveModeViewerProps {
  readonly mode: number | undefined
}

export const ReserveModeViewer: React.FC<ReserveModeViewerProps> = ({mode}) => {
  if (mode === undefined) return <span className={styles.empty}>No mode</span>

  const flags = parseReserveMode(mode)

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

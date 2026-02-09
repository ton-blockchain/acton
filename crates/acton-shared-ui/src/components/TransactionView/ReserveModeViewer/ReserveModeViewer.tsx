import type React from "react"

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
        <span key={flag.value} title={flag.description}>
          {index > 0 && <span className={styles.plus}> + </span>}
          <span className={styles.constant}>
            {flag.name} ({flag.value})
          </span>
        </span>
      ))}
    </div>
  )
}

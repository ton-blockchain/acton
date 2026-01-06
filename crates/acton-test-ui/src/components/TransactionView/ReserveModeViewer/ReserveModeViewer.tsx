import type React from "react"
import styles from "./ReserveModeViewer.module.css"

interface ReserveModeViewerProps {
  readonly mode: number | undefined
}

export const ReserveModeViewer: React.FC<ReserveModeViewerProps> = ({ mode }) => {
  if (mode === undefined) return <span className={styles.empty}>No mode</span>
  return <span className={styles.constant}>Mode ({mode})</span>
}

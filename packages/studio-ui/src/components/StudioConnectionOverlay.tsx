import {WifiOff} from "lucide-react"

import styles from "./StudioConnectionOverlay.module.css"

/** Blocks stale controls while Studio is unreachable and polling attempts recovery. */
export function StudioConnectionOverlay() {
  return (
    <div className={styles.overlay}>
      <div
        className={styles.dialog}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="studio-connection-lost-title"
        aria-describedby="studio-connection-lost-description"
      >
        <div className={styles.icon} aria-hidden="true">
          <WifiOff />
        </div>
        <h1 id="studio-connection-lost-title" className={styles.title}>
          Connection lost
        </h1>
        <p id="studio-connection-lost-description" className={styles.message}>
          The connection to Acton Studio was lost
          <br />
          Restart Studio to continue using the application
        </p>
      </div>
    </div>
  )
}

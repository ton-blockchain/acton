import React from "react"

import styles from "./InlineLoader.module.css"

interface InlineLoaderProps {
  readonly message?: string
  readonly subtext?: string
  readonly loading?: boolean
}

const InlineLoader: React.FC<InlineLoaderProps> = ({
  message = "Loading",
  subtext,
  loading = false,
}) => (
  <div
    className={`${styles.spinnerContainer} ${loading ? "" : styles.hidden}`}
    role="status"
    aria-live="polite"
    aria-hidden={!loading}
  >
    <div className={styles.spinnerWrapper}>
      <div className={styles.spinner} />
      <div className={styles.spinnerGlow} />
    </div>
    {message && <div className={styles.loadingText}>{message}</div>}
    {subtext && (
      <div className={styles.loadingSubtext}>
        <span>{subtext}</span>
        <span className={styles.dotAnimation}>
          <span className={styles.dot}>.</span>
          <span className={styles.dot}>.</span>
          <span className={styles.dot}>.</span>
        </span>
      </div>
    )}
  </div>
)

export default InlineLoader

import {Code2, ExternalLink, X} from "lucide-react"
import {useState} from "react"

import styles from "./DeveloperExplorerBanner.module.css"

const DISMISSED_STORAGE_KEY = "actonExplorerDeveloperBannerDismissed"

const isDismissed = (): boolean => {
  try {
    return sessionStorage.getItem(DISMISSED_STORAGE_KEY) === "true"
  } catch {
    return false
  }
}

export function DeveloperExplorerBanner() {
  const [visible, setVisible] = useState(() => !isDismissed())

  if (!visible) {
    return null
  }

  const dismiss = () => {
    try {
      sessionStorage.setItem(DISMISSED_STORAGE_KEY, "true")
    } catch {
      // The banner can still be dismissed when storage is unavailable.
    }
    setVisible(false)
  }

  return (
    <aside className={styles.banner} aria-label="Developer explorer notice">
      <div className={styles.inner}>
        <p className={styles.message}>
          <Code2 className={styles.developerIcon} size={16} aria-hidden="true" />
          <strong>Acton Explorer is made for smart-contract developers</strong>
          <span className={styles.separator} aria-hidden="true">
            ·
          </span>
          <span>Looking for a public TON explorer?</span>
          <a className={styles.link} href="https://tonscan.org" target="_blank" rel="noreferrer">
            Visit Tonscan
            <ExternalLink size={13} aria-hidden="true" />
          </a>
        </p>
        <button
          type="button"
          className={styles.closeButton}
          aria-label="Dismiss developer explorer notice"
          title="Dismiss"
          onClick={dismiss}
        >
          <X size={16} aria-hidden="true" />
        </button>
      </div>
    </aside>
  )
}

import {Code2, ExternalLink, X} from "lucide-react"
import {useState} from "react"
import {Tooltip} from "@acton/ui"

import styles from "./DeveloperExplorerBanner.module.css"

const DISMISSED_STORAGE_KEY = "actonExplorerDeveloperBannerDismissed"

const isDismissed = (): boolean => {
  try {
    return localStorage.getItem(DISMISSED_STORAGE_KEY) === "true"
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
      localStorage.setItem(DISMISSED_STORAGE_KEY, "true")
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
          <strong>
            <span className={styles.desktopHeadline}>
              Acton Explorer is made for smart-contract developers
            </span>
            <span className={styles.mobileHeadline}>Acton Explorer is made for developers</span>
          </strong>
          <span className={styles.separator} aria-hidden="true">
            ·
          </span>
          <span className={styles.publicExplorerPrompt}>Looking for a public TON explorer?</span>
          <a className={styles.link} href="https://tonscan.org" target="_blank" rel="noreferrer">
            Visit Tonscan
            <ExternalLink size={13} aria-hidden="true" />
          </a>
        </p>
        <Tooltip content="Dismiss">
          <button
            type="button"
            className={styles.closeButton}
            aria-label="Dismiss developer explorer notice"
            onClick={dismiss}
          >
            <X size={16} aria-hidden="true" />
          </button>
        </Tooltip>
      </div>
    </aside>
  )
}

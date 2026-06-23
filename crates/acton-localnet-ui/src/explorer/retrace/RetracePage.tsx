import {useParams} from "react-router-dom"

import {GlobalErrorProvider} from "@retrace/lib/errorContext"
import {useGlobalError} from "@retrace/lib/useGlobalError"
import {ThemeProvider} from "@retrace/lib/themeContext"
import TracePage from "@retrace/pages/TracePage"

import "./retrace.css"
import styles from "./RetracePage.module.css"

export function RetracePage() {
  const {hash = ""} = useParams<{hash: string}>()

  return (
    <GlobalErrorProvider>
      <ThemeProvider>
        <RetraceContent hash={hash} />
      </ThemeProvider>
    </GlobalErrorProvider>
  )
}

function RetraceContent({hash}: {readonly hash: string}) {
  const {error, clearError} = useGlobalError()

  return (
    <div className={styles.root}>
      {error && (
        <div className={styles.errorBanner} role="alert">
          <span>{error}</span>
          <button type="button" onClick={clearError} aria-label="Close retrace error">
            ×
          </button>
        </div>
      )}
      <TracePage initialTx={hash} />
    </div>
  )
}

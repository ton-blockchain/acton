import {Button} from "@acton/ui"
import {CircleAlert} from "lucide-react"
import type {ReactNode} from "react"

import styles from "./TablePage.module.css"

interface TablePageProps {
  readonly children: ReactNode
  readonly error?: string
  readonly errorTitle: string
  readonly hasContent: boolean
  readonly onRetry: () => Promise<void>
}

export function TablePage({children, error, errorTitle, hasContent, onRetry}: TablePageProps) {
  return (
    <div className={styles.page}>
      {error && !hasContent ? (
        <section className={styles.errorPanel} aria-live="polite">
          <CircleAlert size={18} aria-hidden="true" />
          <div>
            <strong>{errorTitle}</strong>
            <span>{error}</span>
          </div>
          <Button size="sm" variant="outline" onClick={() => void onRetry()}>
            Retry
          </Button>
        </section>
      ) : (
        children
      )}
    </div>
  )
}

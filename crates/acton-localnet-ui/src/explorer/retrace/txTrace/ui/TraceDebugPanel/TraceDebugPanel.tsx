import type {CSSProperties, ReactNode} from "react"
import {X} from "lucide-react"

import InlineLoader from "../InlineLoader"
import {useAvailableFlowMetrics} from "../../../../hooks/useAvailableFlowMetrics"
import "../../../Retrace.tokens.css"
import styles from "./TraceDebugPanel.module.css"

const MAX_RETRACE_FLOW_WIDTH = 1800

interface TraceDebugPanelProps {
  readonly children: ReactNode
  readonly className?: string
  readonly title?: string
  readonly onClose: () => void
}

export function TraceDebugPanel({
  children,
  className,
  title = "Debug",
  onClose,
}: TraceDebugPanelProps) {
  const {flowMetrics, rootRef} = useAvailableFlowMetrics<HTMLDivElement>(MAX_RETRACE_FLOW_WIDTH)
  const rootStyle = {
    "--retrace-flow-offset": `${flowMetrics.offset}px`,
    "--retrace-flow-width": flowMetrics.width > 0 ? `${flowMetrics.width}px` : "100vw",
  } as CSSProperties

  return (
    <div
      ref={rootRef}
      className={`${styles.root} ${className ?? ""} retraceRoot`}
      style={rootStyle}
    >
      <div className={styles.header}>
        <div className={styles.title}>{title}</div>
        <button
          type="button"
          className={styles.closeButton}
          onClick={onClose}
          aria-label="Close debug panel"
        >
          <X size={16} />
        </button>
      </div>

      <div className={styles.content}>{children}</div>
    </div>
  )
}

export function TraceDebugPanelLoading() {
  return (
    <div className={styles.loadingState}>
      <InlineLoader
        message="Tracing transaction"
        subtext="This may take a few moments"
        loading={true}
      />
    </div>
  )
}

export function TraceDebugPanelError({
  title,
  message,
}: {
  readonly title: string
  readonly message: string
}) {
  return (
    <div className={styles.errorState}>
      <div className={styles.errorTitle}>{title}</div>
      <div className={styles.errorMessage}>{message}</div>
    </div>
  )
}

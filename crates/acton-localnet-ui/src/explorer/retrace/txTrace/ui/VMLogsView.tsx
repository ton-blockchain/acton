import React, {useState} from "react"
import {FiChevronDown, FiChevronRight, FiCopy, FiCheck} from "react-icons/fi"

import Button from "@retrace/ui/Button"

import styles from "./VMLogsView.module.css"

export interface VMLogsViewProps {
  readonly logs: string | undefined
  readonly title?: string
  readonly isExpandable?: boolean
  readonly defaultExpanded?: boolean
}

const VMLogsView: React.FC<VMLogsViewProps> = ({
  logs,
  title = "Logs",
  isExpandable = false,
  defaultExpanded = false,
}) => {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded)
  const [copyStatus, setCopyStatus] = useState<"idle" | "copied">("idle")

  if (!logs) {
    return null
  }

  const toggleExpand = () => {
    if (isExpandable) {
      setIsExpanded(!isExpanded)
    }
  }

  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (isExpandable && (event.key === "Enter" || event.key === " ")) {
      toggleExpand()
    }
  }

  const handleCopy = async () => {
    if (!logs) return
    try {
      await navigator.clipboard.writeText(logs)
      setCopyStatus("copied")
      setTimeout(() => setCopyStatus("idle"), 1500)
    } catch (err) {
      console.error("Failed to copy logs:", err)
    }
  }

  const renderLogContent = () => (
    <div className={styles.logsContentInner}>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => {
          void handleCopy()
        }}
        className={styles.copyButton}
        aria-label={copyStatus === "idle" ? "Copy logs" : "Logs copied"}
      >
        {copyStatus === "idle" ? <FiCopy /> : <FiCheck />}
      </Button>
      <pre className={styles.vmLogs}>{logs}</pre>
    </div>
  )

  if (isExpandable) {
    return (
      <div className={styles.expandableContainer}>
        <div
          className={styles.expandableHeader}
          onClick={toggleExpand}
          onKeyDown={handleKeyDown}
          role="button"
          tabIndex={0}
          aria-expanded={isExpanded}
        >
          {isExpanded ? <FiChevronDown /> : <FiChevronRight />}
          <h3 className={styles.logsHeading}>{title}</h3>
        </div>
        {isExpanded && <div className={styles.logsContent}>{renderLogContent()}</div>}
      </div>
    )
  }

  return (
    <div className={styles.logsContainer}>
      {title && <h3 className={styles.logsHeading}>{title}</h3>}
      {renderLogContent()}
    </div>
  )
}

export default VMLogsView

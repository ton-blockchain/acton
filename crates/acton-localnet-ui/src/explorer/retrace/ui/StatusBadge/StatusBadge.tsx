import React from "react"

import {Tooltip} from "@retrace/ui/Tooltip"
import {EXIT_CODE_DESCRIPTIONS} from "@retrace/common/lib/error-codes/error-codes"
import exitStyles from "@retrace/common/ui/ExitCodeChip/ExitCodeViewer.module.css"

import styles from "./StatusBadge.module.css"

export type StatusType = "success" | "failed" | "warning"

export interface StatusBadgeProps {
  readonly type: StatusType
  readonly text?: string
  readonly exitCode?: number
}

const StatusBadge: React.FC<StatusBadgeProps> = ({type, text, exitCode}) => {
  const getAriaLabel = () => {
    const statusText = text ?? type
    switch (type) {
      case "success":
        return `Success: ${statusText}`
      case "failed":
        return `Error: ${statusText}`
      case "warning":
        return `Warning: ${statusText}`
      default:
        return statusText
    }
  }

  if (type === "success") {
    const tooltipContent = (
      <div className={exitStyles.tooltipContent}>
        <div className={exitStyles.tooltipSection}>
          <div className={exitStyles.tooltipLabel}>Description:</div>
          <div className={exitStyles.tooltipDescription}>Transaction completed successfully</div>
        </div>
      </div>
    )
    return (
      <Tooltip content={tooltipContent} placement="bottom">
        <span
          className={styles.statusSuccess}
          role="status"
          aria-label={getAriaLabel()}
          data-testid="status-badge"
        >
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
            aria-hidden="true"
          >
            <path
              d="M22 11.08V12C21.9988 14.1564 21.3005 16.2547 20.0093 17.9818C18.7182 19.709 16.9033 20.9725 14.8354 21.5839C12.7674 22.1953 10.5573 22.1219 8.53447 21.3746C6.51168 20.6273 4.78465 19.2461 3.61096 17.4371C2.43727 15.628 1.87979 13.4881 2.02168 11.3363C2.16356 9.18455 2.99721 7.13631 4.39828 5.49706C5.79935 3.85781 7.69279 2.71537 9.79619 2.24013C11.8996 1.7649 14.1003 1.98232 16.07 2.85999"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <path
              d="M22 4L12 14.01L9 11.01"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          {text ?? "Success"}
        </span>
      </Tooltip>
    )
  }if (type === "failed") {
    const info =
      exitCode === undefined
        ? undefined
        : EXIT_CODE_DESCRIPTIONS[exitCode as keyof typeof EXIT_CODE_DESCRIPTIONS]
    const description = info?.description
    const phase = info?.phase
    const displayName = info?.name ?? "Custom error"
    const tooltipContent = (
      <div className={exitStyles.tooltipContent}>
        <div className={exitStyles.tooltipSection}>
          <div className={exitStyles.tooltipLabel}>Description:</div>
          <div className={exitStyles.tooltipDescription}>
            {exitCode === undefined ? "Unknown error" : displayName}
            {description ? `: ${description}` : ""}
          </div>
        </div>
        {phase && (
          <div className={exitStyles.tooltipSection}>
            <div className={exitStyles.tooltipLabel}>Origin:</div>
            <div className={exitStyles.tooltipPhase}>{phase}</div>
          </div>
        )}
      </div>
    )
    return (
      <Tooltip content={tooltipContent} placement="bottom">
        <span
          className={styles.statusFailed}
          role="status"
          aria-label={getAriaLabel()}
          data-testid="status-badge"
        >
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
            aria-hidden="true"
          >
            <path
              d="M12 22C17.5228 22 22 17.5228 22 12C22 6.47715 17.5228 2 12 2C6.47715 2 2 6.47715 2 12C2 17.5228 6.47715 22 12 22Z"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <path
              d="M15 9L9 15"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <path
              d="M9 9L15 15"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          {text ?? "Failed"}
        </span>
      </Tooltip>
    )
  }if (type === "warning") {
    return (
      <span
        className={styles.statusWarning}
        role="status"
        aria-label={getAriaLabel()}
        data-testid="status-badge"
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
          <line x1="12" y1="9" x2="12" y2="13"></line>
          <line x1="12" y1="17" x2="12.01" y2="17"></line>
        </svg>
        {text ?? "Warning"}
      </span>
    )
  }

  return null
}

export default StatusBadge

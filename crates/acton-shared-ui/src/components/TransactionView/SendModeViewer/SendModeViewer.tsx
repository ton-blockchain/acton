import type * as React from "react"

import {Tooltip} from "@/components/Tooltip/Tooltip"
import {parseSendMode} from "@/components/TransactionView/SendModeViewer/parser"

import styles from "./SendModeViewer.module.css"

export type {SendModeInfo} from "@/components/TransactionView/SendModeViewer/parser"

interface SendModeViewerProps {
  readonly mode: number | undefined
}

export const SendModeViewer: React.FC<SendModeViewerProps> = ({mode}) => {
  if (mode === undefined) {
    return <span className={styles.empty}>No mode</span>
  }

  const flags = parseSendMode(mode)

  return (
    <div className={styles.container}>
      {flags.map((flag, index) => (
        <span key={`${flag.name}-${flag.value}`} className={styles.modeItem}>
          {index > 0 && <span className={styles.plus}> + </span>}
          <Tooltip
            content={<div className={styles.tooltipDescription}>{flag.description}</div>}
            variant="hover"
          >
            <span className={styles.constant}>
              {flag.name} ({flag.value})
            </span>
          </Tooltip>
        </span>
      ))}
    </div>
  )
}

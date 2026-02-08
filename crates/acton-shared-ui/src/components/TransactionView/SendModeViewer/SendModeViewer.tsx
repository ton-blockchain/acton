import * as React from "react"

import {parseSendMode} from "@/components/TransactionView/SendModeViewer/parser"

import styles from "./SendModeViewer.module.css"

export interface SendModeInfo {
  readonly name: string
  readonly value: number
  readonly description: string
}

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
        <span key={flag.value} title={flag.description}>
          {index > 0 && <span className={styles.plus}> + </span>}
          <span className={styles.constant}>
            {flag.name} ({flag.value})
          </span>
        </span>
      ))}
    </div>
  )
}

import React, {useState} from "react"

import {FiCheck, FiCopy} from "react-icons/fi"

import Button from "@retrace/ui/Button"

import styles from "./DataBlock.module.css"

interface DataBlockProps {
  readonly data: string
  readonly label?: string
  readonly maxHeight?: number
}

export const DataBlock: React.FC<DataBlockProps> = ({data, label, maxHeight = 100}) => {
  const [copyStatus, setCopyStatus] = useState<"idle" | "copied">("idle")

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(data)
      setCopyStatus("copied")
      setTimeout(() => setCopyStatus("idle"), 1500)
    } catch (err) {
      console.error("Failed to copy data:", err)
    }
  }

  return (
    <div className={styles.container}>
      {label && <div className={styles.label}>{label}</div>}
      <div className={styles.contentWrapper}>
        <pre className={styles.content} style={{maxHeight: `${maxHeight}px`}}>
          {data}
        </pre>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => {
            void handleCopy()
          }}
          className={styles.copyButton}
          aria-label={copyStatus === "idle" ? "Copy" : "Copied"}
        >
          {copyStatus === "idle" ? <FiCopy /> : <FiCheck />}
        </Button>
      </div>
    </div>
  )
}

export default DataBlock

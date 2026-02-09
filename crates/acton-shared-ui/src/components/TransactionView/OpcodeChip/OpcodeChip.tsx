import type React from "react"
import {useCallback, useEffect, useState} from "react"

import styles from "./OpcodeChip.module.css"

interface OpcodeChipProps {
  readonly opcode?: number
  readonly abiName?: string
  readonly showOpcode?: boolean
}

export const OpcodeChip: React.FC<OpcodeChipProps> = ({opcode, abiName, showOpcode = false}) => {
  const [isCopied, setIsCopied] = useState(false)

  const displayText = abiName ?? (opcode ? `0x${opcode.toString(16)}` : "Empty")
  const displaySubText = abiName && showOpcode && opcode ? `0x${opcode.toString(16)}` : undefined
  const copyValue = opcode ? `0x${opcode.toString(16)}` : ""

  const handleCopy = useCallback(
    async (event: React.MouseEvent) => {
      event.stopPropagation()
      if (!copyValue) return

      try {
        await navigator.clipboard.writeText(copyValue)
        setIsCopied(true)
      } catch (error) {
        console.error("Failed to copy:", error)
      }
    },
    [copyValue],
  )

  useEffect(() => {
    if (isCopied) {
      const timer = setTimeout(() => setIsCopied(false), 2000)
      return () => clearTimeout(timer)
    }
  }, [isCopied])

  return (
    <div className={styles.opcodeChip}>
      <span className={styles.opcodeText}>{displayText}</span>
      {displaySubText && <span className={styles.opcodeSubText}>{`··${displaySubText}`}</span>}
      {copyValue && (
        <button
          type="button"
          className={styles.copyButton}
          onClick={event => {
            handleCopy(event).catch(console.error)
          }}
          title={`Copy ${copyValue}`}
          aria-label={`Copy opcode ${copyValue}`}
        >
          {isCopied ? (
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <title>Copied</title>
              <polyline points="20,6 9,17 4,12" />
            </svg>
          ) : (
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <title>Copy opcode</title>
              <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
              <path d="m5,15 L5,5 a2,2 0 0,1 2,-2 l10,0" />
            </svg>
          )}
        </button>
      )}
    </div>
  )
}

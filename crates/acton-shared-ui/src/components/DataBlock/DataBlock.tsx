import {Check, ChevronDown, ChevronRight, Copy} from "lucide-react"
import type React from "react"
import {useEffect, useState} from "react"
import {clsx} from "clsx"

import styles from "./DataBlock.module.css"

export type DataBlockVariant = "inline" | "standalone"

export interface DataBlockProps {
  readonly data?: string
  readonly children?: React.ReactNode
  readonly className?: string
  readonly contentClassName?: string
  readonly label?: string
  readonly collapsible?: boolean
  readonly copyLabel?: string
  readonly copyValue?: string
  readonly defaultExpanded?: boolean
  readonly maxHeight?: number | string
  readonly showCopy?: boolean
  readonly variant?: DataBlockVariant
  readonly wrap?: boolean
  readonly visualDynamic?: string
  readonly visualPlaceholder?: string
}

export const DataBlock: React.FC<DataBlockProps> = ({
  data,
  children,
  className,
  contentClassName,
  label,
  collapsible = false,
  copyLabel,
  copyValue,
  defaultExpanded = true,
  maxHeight,
  showCopy = true,
  variant = "inline",
  wrap = true,
  visualDynamic,
  visualPlaceholder,
}) => {
  const [isCopied, setIsCopied] = useState(false)
  const [isExpanded, setIsExpanded] = useState(defaultExpanded)
  const valueToCopy = copyValue ?? data
  const resolvedCopyLabel = copyLabel ?? label ?? "data"
  const maxHeightValue = typeof maxHeight === "number" ? `${maxHeight}px` : maxHeight
  const copyTitle = isCopied ? `Copied ${resolvedCopyLabel}` : `Copy ${resolvedCopyLabel}`
  const isContentVisible = !collapsible || isExpanded
  const headerLabel = label ?? copyLabel ?? "Data"

  useEffect(() => {
    if (!isCopied) {
      return
    }

    const timer = setTimeout(() => setIsCopied(false), 1600)
    return () => clearTimeout(timer)
  }, [isCopied])

  const handleCopy = (event: React.MouseEvent<HTMLButtonElement>): void => {
    event.stopPropagation()

    if (!valueToCopy) {
      return
    }

    navigator.clipboard
      .writeText(valueToCopy)
      .then(() => setIsCopied(true))
      .catch((error: unknown) => {
        console.error("Failed to copy data:", error)
      })
  }

  return (
    <div
      className={clsx(
        styles.container,
        variant === "standalone" ? styles.standalone : styles.inline,
        collapsible && styles.collapsible,
        className,
      )}
      data-visual-dynamic={visualDynamic}
      data-visual-placeholder={visualPlaceholder}
    >
      {label && !collapsible && <div className={styles.label}>{label}</div>}
      <div className={styles.contentWrapper}>
        {collapsible && (
          <button
            type="button"
            className={styles.collapsibleHeader}
            onClick={() => setIsExpanded(current => !current)}
            aria-expanded={isExpanded}
          >
            {isExpanded ? (
              <ChevronDown size={16} aria-hidden="true" />
            ) : (
              <ChevronRight size={16} aria-hidden="true" />
            )}
            <span>{headerLabel}</span>
          </button>
        )}
        {isContentVisible && (
          <div className={styles.contentBody}>
            {children ? (
              <div
                className={clsx(styles.childrenContent, contentClassName)}
                style={maxHeightValue ? {maxHeight: maxHeightValue} : undefined}
              >
                {children}
              </div>
            ) : (
              <pre
                className={clsx(styles.content, wrap && styles.contentWrap, contentClassName)}
                style={maxHeightValue ? {maxHeight: maxHeightValue} : undefined}
              >
                {data}
              </pre>
            )}
            {showCopy && valueToCopy && (
              <button
                type="button"
                className={styles.copyButton}
                onClick={handleCopy}
                aria-label={copyTitle}
                title={copyTitle}
              >
                {isCopied ? (
                  <Check size={14} aria-hidden="true" />
                ) : (
                  <Copy size={14} aria-hidden="true" />
                )}
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  )
}

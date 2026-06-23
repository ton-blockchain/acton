import {useCallback, useEffect, useState} from "react"

import styles from "./CodeBlock.module.css"

export interface CodeBlockProps {
  readonly title?: string
  readonly content: string | undefined | null
  readonly variant?: "assembly" | "hex" | "small-hex"
  readonly placeholder?: string
  readonly className?: string
}

const CopyIcon = () => (
  <svg
    width="14"
    height="14"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
  </svg>
)

const CheckIcon = () => (
  <svg
    width="14"
    height="14"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <polyline points="20 6 9 17 4 12"></polyline>
  </svg>
)

export function CodeBlock({
  title,
  content,
  variant = "hex",
  placeholder,
  className,
}: CodeBlockProps) {
  const [isCopied, setIsCopied] = useState(false)

  const displayContent = content ?? placeholder ?? "No content available"

  const handleCopy = useCallback(() => {
    if (!content) return

    navigator.clipboard
      .writeText(content)
      .then(() => {
        setIsCopied(true)
      })
      .catch(err => {
        console.error("Failed to copy: ", err)
      })
  }, [content])

  useEffect(() => {
    if (isCopied) {
      const timer = setTimeout(() => {
        setIsCopied(false)
      }, 1500)
      return () => clearTimeout(timer)
    }
  }, [isCopied])

  const contentClassName =
    variant === "assembly"
      ? `${styles.content} ${styles.contentAssembly}`
      : variant === "small-hex"
        ? `${styles.content} ${styles.contentSmallHex}`
        : `${styles.content} ${styles.contentHex}`

  return (
    <div className={`${styles.container} ${className ?? ""}`}>
      <div className={styles.header}>
        {title && <div className={styles.title}>{title}</div>}
        {content && (
          <button
            onClick={handleCopy}
            className={`${styles.copyButton} ${isCopied ? styles.copied : ""}`}
            title={isCopied ? "Copied!" : "Copy"}
            aria-label={isCopied ? "Copied to clipboard" : "Copy to clipboard"}
            disabled={isCopied}
            type="button"
          >
            {isCopied ? <CheckIcon /> : <CopyIcon />}
          </button>
        )}
      </div>
      <div className={contentClassName}>{displayContent}</div>
    </div>
  )
}

import React, {useCallback, useEffect, useState} from "react"

import Icon from "@retrace/ui/Icon"

import styles from "./CopyButton.module.css"

export const CopyButton: React.FC<{
  readonly value: string
  readonly title: string
  readonly className?: string
}> = ({value, title, className}) => {
  const [isCopied, setIsCopied] = useState(false)

  const handleCopy = useCallback(() => {
    navigator.clipboard
      .writeText(value)
      .then(() => {
        setIsCopied(true)
      })
      .catch(err => {
        console.error("Failed to copy: ", err)
      })
  }, [value])

  useEffect(() => {
    if (isCopied) {
      const timer = setTimeout(() => {
        setIsCopied(false)
      }, 1500)
      return () => clearTimeout(timer)
    }
  }, [isCopied])

  const copyIconSvg = (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
    </svg>
  )

  const checkIconSvg = (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <polyline points="20 6 9 17 4 12"></polyline>
    </svg>
  )

  return (
    <>
      <button
        onClick={e => {
          e.stopPropagation()
          handleCopy()
        }}
        className={`${styles.copyIcon} ${isCopied ? styles.copied : ""} ${className ?? ""}`}
        title={isCopied ? "Copied!" : title}
        aria-label={isCopied ? "Copied to clipboard" : `Copy ${title.toLowerCase()}`}
        disabled={isCopied}
        type="button"
      >
        <Icon svg={isCopied ? checkIconSvg : copyIconSvg} />
      </button>

      <div className="sr-only" aria-live="polite" aria-atomic="true">
        {isCopied && "Copied to clipboard"}
      </div>
    </>
  )
}

import type React from "react"
import {useEffect, useState} from "react"

interface CopyValueButtonProps {
  readonly className: string
  readonly value: string
  readonly label?: string
  readonly caption?: string
}

export function CopyValueButton({
  className,
  value,
  label = "full BOC hex",
  caption,
}: CopyValueButtonProps): React.JSX.Element {
  const [isCopied, setIsCopied] = useState(false)
  const title = isCopied ? `Copied ${label}` : `Copy ${label}`

  const handleCopy = (): void => {
    navigator.clipboard
      .writeText(value)
      .then(() => {
        setIsCopied(true)
      })
      .catch((error: unknown) => {
        console.error("Failed to copy:", error)
      })
  }

  useEffect(() => {
    if (!isCopied) {
      return
    }

    const timer = setTimeout(() => setIsCopied(false), 2000)
    return () => clearTimeout(timer)
  }, [isCopied])

  return (
    <button
      type="button"
      className={className}
      onClick={event => {
        event.stopPropagation()
        handleCopy()
      }}
      title={title}
      aria-label={title}
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
          <title>{title}</title>
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
          <path d="m5,15 L5,5 a2,2 0 0,1 2,-2 l10,0" />
        </svg>
      )}
      {caption && <span>{isCopied ? "Copied" : caption}</span>}
    </button>
  )
}

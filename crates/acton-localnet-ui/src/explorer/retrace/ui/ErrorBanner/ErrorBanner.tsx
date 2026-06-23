import React, {useEffect, useState, useCallback} from "react"

import Button from "@retrace/ui/Button"

import styles from "./ErrorBanner.module.css"

interface Props {
  readonly message: string
  readonly onClose: () => void
}

const ErrorBanner: React.FC<Props> = ({message, onClose}) => {
  const [isClosing, setIsClosing] = useState(false)
  const [isHovered, setIsHovered] = useState(false)

  const handleClose = useCallback(() => {
    setIsClosing(true)
    const timer = setTimeout(onClose, 300)
    return () => clearTimeout(timer)
  }, [onClose])

  useEffect(() => {
    if (!isHovered) {
      const timer = setTimeout(handleClose, 5000)
      return () => clearTimeout(timer)
    }
  }, [handleClose, isHovered])

  return (
    <div
      className={`${styles.errorBanner} ${isClosing ? styles.slideOut : ""}`}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      role="alert"
      aria-live="assertive"
      aria-atomic="true"
      data-testid="error-banner-info"
    >
      <span>{message}</span>
      <Button
        variant="ghost"
        size="sm"
        onClick={handleClose}
        aria-label="Close error message"
        title="Close error message (Esc)"
      >
        <span aria-hidden="true">×</span>
      </Button>
    </div>
  )
}

export default ErrorBanner

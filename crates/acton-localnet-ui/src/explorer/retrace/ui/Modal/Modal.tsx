import React, {useEffect} from "react"

import styles from "./Modal.module.css" // Import CSS module

interface ModalProps {
  readonly open: boolean
  readonly onClose: () => void
  readonly children: React.ReactNode
  readonly contentClassName?: string
}

const Modal: React.FC<ModalProps> = ({open, onClose, children, contentClassName}) => {
  useEffect(() => {
    const handleEscKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose()
      }
    }

    if (open) {
      document.addEventListener("keydown", handleEscKey)
    }

    return () => {
      document.removeEventListener("keydown", handleEscKey)
    }
  }, [open, onClose])

  if (!open) return null

  const handleOverlayKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Enter" || event.key === " ") {
      onClose()
    }
  }

  const modalContentClasses = `${styles.modalContent} ${contentClassName || ""}`.trim()

  return (
    <div
      className={styles.modalOverlay}
      onClick={onClose}
      onKeyDown={handleOverlayKeyDown}
      role="button"
      tabIndex={0}
      aria-label="Close modal"
    >
      {/* eslint-disable jsx-a11y/no-static-element-interactions, jsx-a11y/click-events-have-key-events */}
      <div className={modalContentClasses} onClick={e => e.stopPropagation()}>
        {children}
      </div>
      {/* eslint-enable jsx-a11y/no-static-element-interactions, jsx-a11y/click-events-have-key-events */}
    </div>
  )
}

export default Modal

import * as React from "react"
import {createPortal} from "react-dom"
import styles from "./ToastProvider.module.css"
import {ToastContext, type ToastContextValue, type ToastOptions} from "./useToast"

interface ToastRecord extends ToastOptions {
  readonly id: string
  readonly variant: "info" | "success" | "error"
  readonly durationMs: number
}

const DEFAULT_DURATION_MS = 4000

function createToastId(): string {
  if ("randomUUID" in crypto) {
    return crypto.randomUUID()
  }

  return `${Date.now()}-${Math.random().toString(36).slice(2)}`
}

function ToastViewport({
  toasts,
  onDismiss,
}: {
  readonly toasts: readonly ToastRecord[]
  readonly onDismiss: (id: string) => void
}): React.JSX.Element | undefined {
  const [mounted, setMounted] = React.useState(false)

  React.useEffect(() => {
    setMounted(true)
  }, [])

  if (!mounted || toasts.length === 0) {
    return undefined
  }

  return createPortal(
    <div className={styles.viewport} aria-live="polite" aria-atomic="true">
      {toasts.map(toast => (
        <ToastItem key={toast.id} toast={toast} onDismiss={onDismiss} />
      ))}
    </div>,
    document.body,
  )
}

function ToastItem({
  toast,
  onDismiss,
}: {
  readonly toast: ToastRecord
  readonly onDismiss: (id: string) => void
}): React.JSX.Element {
  const [isPaused, setIsPaused] = React.useState(false)
  const [isExiting, setIsExiting] = React.useState(false)

  React.useEffect(() => {
    if (isPaused || isExiting) {
      return
    }

    const timer = globalThis.setTimeout(() => {
      setIsExiting(true)
    }, toast.durationMs)

    return () => {
      globalThis.clearTimeout(timer)
    }
  }, [isExiting, isPaused, toast.durationMs])

  React.useEffect(() => {
    if (!isExiting) {
      return
    }

    const timer = globalThis.setTimeout(() => {
      onDismiss(toast.id)
    }, 180)

    return () => {
      globalThis.clearTimeout(timer)
    }
  }, [isExiting, onDismiss, toast.id])

  return (
    <div
      className={`${styles.toast} ${styles[`toast${capitalize(toast.variant)}`]} ${
        isExiting ? styles.toastExiting : ""
      }`}
      role="status"
      onMouseEnter={() => setIsPaused(true)}
      onMouseLeave={() => setIsPaused(false)}
    >
      <div className={styles.toastBody}>
        {toast.title ? <div className={styles.toastTitle}>{toast.title}</div> : undefined}
        <div className={styles.toastDescription}>{toast.description}</div>
      </div>
      <button
        type="button"
        className={styles.dismissButton}
        aria-label="Dismiss notification"
        onClick={() => onDismiss(toast.id)}
      >
        <span className={styles.dismissBar} />
        <span className={styles.dismissBar} />
      </button>
    </div>
  )
}

function capitalize(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1)
}

export const ToastProvider: React.FC<React.PropsWithChildren> = ({children}) => {
  const [toasts, setToasts] = React.useState<readonly ToastRecord[]>([])

  const dismissToast = React.useCallback((id: string) => {
    setToasts(current => current.filter(toast => toast.id !== id))
  }, [])

  const showToast = React.useCallback((options: ToastOptions) => {
    const id = createToastId()
    const toast: ToastRecord = {
      id,
      title: options.title,
      description: options.description,
      variant: options.variant ?? "info",
      durationMs: options.durationMs ?? DEFAULT_DURATION_MS,
    }

    setToasts(current => [...current, toast])
    return id
  }, [])

  const contextValue = React.useMemo<ToastContextValue>(
    () => ({
      showToast,
      dismissToast,
    }),
    [dismissToast, showToast],
  )

  return (
    <ToastContext.Provider value={contextValue}>
      {children}
      <ToastViewport toasts={toasts} onDismiss={dismissToast} />
    </ToastContext.Provider>
  )
}

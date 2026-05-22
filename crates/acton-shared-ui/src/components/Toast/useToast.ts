import * as React from "react"

type ToastVariant = "info" | "success" | "error"

export interface ToastOptions {
  readonly title?: string
  readonly description: string
  readonly variant?: ToastVariant
  readonly durationMs?: number
}

export interface ToastContextValue {
  readonly showToast: (options: ToastOptions) => void
  readonly dismissToast: (id: string) => void
}

export const ToastContext = React.createContext<ToastContextValue | undefined>(undefined)

export function useToast(): ToastContextValue {
  const context = React.useContext(ToastContext)
  if (!context) {
    throw new Error("useToast must be used within ToastProvider")
  }
  return context
}

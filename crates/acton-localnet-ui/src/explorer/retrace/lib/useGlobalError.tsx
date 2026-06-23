import {createContext, useContext} from "react"

import type {GlobalErrorContextValue} from "@retrace/lib/errorContext"

export const GlobalErrorContext = createContext<GlobalErrorContextValue | undefined>(undefined)

export function useGlobalError(): GlobalErrorContextValue {
  const ctx = useContext(GlobalErrorContext)
  if (!ctx) {
    throw new Error("useGlobalError must be used within GlobalErrorProvider")
  }
  return ctx
}

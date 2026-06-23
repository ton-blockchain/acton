import React, {useState} from "react"

import {GlobalErrorContext} from "@retrace/lib/useGlobalError"

export interface GlobalErrorContextValue {
  readonly error: string | null
  readonly setError: (message: string) => void
  readonly clearError: () => void
}

export const GlobalErrorProvider: React.FC<{children: React.ReactNode}> = ({children}) => {
  const [error, setErrorState] = useState<string | null>(null)

  const setError = (message: string) => setErrorState(message)
  const clearError = () => setErrorState(null)

  return (
    <GlobalErrorContext.Provider value={{error, setError, clearError}}>
      {children}
    </GlobalErrorContext.Provider>
  )
}

import {useMemo} from "react"
import type {ReactNode} from "react"

import {TestUiApiBaseContext} from "./testUiApiContext"

interface TestUiApiProviderProps {
  readonly baseUrl: string
  readonly children: ReactNode
}

export function TestUiApiProvider({baseUrl, children}: TestUiApiProviderProps) {
  const normalizedBaseUrl = useMemo(
    () => (baseUrl.endsWith("/") ? baseUrl.slice(0, -1) : baseUrl),
    [baseUrl],
  )

  return (
    <TestUiApiBaseContext.Provider value={normalizedBaseUrl}>
      {children}
    </TestUiApiBaseContext.Provider>
  )
}

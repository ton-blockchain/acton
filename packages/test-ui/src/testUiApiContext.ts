import {createContext, useCallback, useContext} from "react"

export const TestUiApiBaseContext = createContext("/api")

export function useTestUiApi() {
  const baseUrl = useContext(TestUiApiBaseContext)
  const url = useCallback(
    (path: string) => `${baseUrl}${path.startsWith("/") ? path : `/${path}`}`,
    [baseUrl],
  )

  return {baseUrl, url}
}

import {useEffect, useState} from "react"

export function useDelayedLoadingVisibility(isLoading: boolean, delayMs: number): boolean {
  const [isVisible, setIsVisible] = useState(false)

  useEffect(() => {
    if (!isLoading) {
      setIsVisible(false)
      return
    }

    const timeout = globalThis.setTimeout(() => setIsVisible(true), delayMs)
    return () => globalThis.clearTimeout(timeout)
  }, [delayMs, isLoading])

  return isVisible
}

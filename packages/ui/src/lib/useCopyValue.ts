import {useEffect, useState} from "react"

export type UseCopyValueOptions = Readonly<{
  readonly onCopy?: (value: string) => Promise<void> | void
  readonly onCopyError?: (error: unknown) => void
  readonly resetDelay?: number
  readonly value: string
}>

export function useCopyValue({onCopy, onCopyError, resetDelay = 2000, value}: UseCopyValueOptions) {
  const [isCopied, setIsCopied] = useState(false)

  useEffect(() => {
    setIsCopied(false)
  }, [value])

  useEffect(() => {
    if (!isCopied || resetDelay <= 0) return

    const timer = globalThis.setTimeout(() => setIsCopied(false), resetDelay)
    return () => globalThis.clearTimeout(timer)
  }, [isCopied, resetDelay])

  const copy = async () => {
    try {
      if (onCopy) {
        await onCopy(value)
      } else {
        await navigator.clipboard.writeText(value)
      }

      setIsCopied(true)
    } catch (error) {
      onCopyError?.(error)
    }
  }

  return {copy, isCopied} as const
}

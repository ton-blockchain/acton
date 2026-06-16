import {useEffect} from "react"

function isPotentialTonConnectUrl(value: string): boolean {
  const normalized = value.trim().toLowerCase()

  if (
    normalized.startsWith("tonconnect://") ||
    normalized.startsWith("tc://") ||
    normalized.startsWith("ton://")
  ) {
    return true
  }

  if (!normalized.startsWith("https://") && !normalized.startsWith("http://")) {
    return false
  }

  try {
    const url = new URL(value)
    const pathSegments = url.pathname
      .toLowerCase()
      .split("/")
      .filter(segment => segment.length > 0)
    const searchEntries = [...url.searchParams.entries()]

    return (
      pathSegments.some(segment => segment === "ton-connect" || segment === "tonconnect") ||
      searchEntries.some(([key, entryValue]) => {
        const normalizedKey = key.toLowerCase()
        const normalizedValue = entryValue.toLowerCase()

        return (
          normalizedKey === "ton-connect" ||
          normalizedKey === "tonconnect" ||
          normalizedValue.includes("tonconnect://") ||
          normalizedValue.includes("tc://")
        )
      }) ||
      (url.searchParams.has("v") && url.searchParams.has("id") && url.searchParams.has("r"))
    )
  } catch {
    return false
  }
}

export function useTonConnectPasteHandler(
  handleTonConnectUrl: (url: string) => Promise<void>,
): void {
  useEffect(() => {
    const handlePaste = (event: ClipboardEvent) => {
      if (event.defaultPrevented) {
        return
      }

      const pastedText = event.clipboardData?.getData("text")?.trim()
      if (!pastedText || !isPotentialTonConnectUrl(pastedText)) {
        return
      }

      void handleTonConnectUrl(pastedText)
    }

    document.addEventListener("paste", handlePaste)
    return () => document.removeEventListener("paste", handlePaste)
  }, [handleTonConnectUrl])
}

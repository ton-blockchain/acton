export function getRawQueryParam(name: string): string | null {
  const search = window.location.search
  if (!search || search.length <= 1) return null
  const query = search.slice(1)
  const pairs = query.split("&")
  for (const pair of pairs) {
    if (!pair) continue
    const eq = pair.indexOf("=")
    const key = eq >= 0 ? pair.slice(0, eq) : pair
    if (key !== name) continue
    const raw = eq >= 0 ? pair.slice(eq + 1) : ""
    try {
      return decodeURIComponent(raw)
    } catch {
      return raw
    }
  }
  return null
}

export function setQueryParam(name: string, value: string | null): void {
  const url = new URL(window.location.href)
  if (value === null) {
    url.searchParams.delete(name)
  } else {
    url.searchParams.set(name, value)
  }
  window.history.replaceState({}, "", url.toString())
}

import { Address } from "@ton/core"
export function parseAddress(address: string): Address | null {
  if (!address) return null
  try {
    return Address.parse(address)
  } catch {
    return null
  }
}

export function toTestnetAddress(address: string): string | null {
  const parsed = parseAddress(address)
  return parsed ? parsed.toString({ testOnly: true }) : null
}

export function normalizeAddress(address: string): string {
  return toTestnetAddress(address) ?? address
}

export function isSameAddress(a: string, b: string): boolean {
  if (!a || !b) return false
  const parsedA = parseAddress(a)
  const parsedB = parseAddress(b)
  if (parsedA && parsedB) return parsedA.equals(parsedB)
  return a === b
}

export function formatNano(nano: string | number): string {
  const n = typeof nano === "string" ? BigInt(nano) : BigInt(nano)
  const ton = Number(n) / 1e9
  return ton.toLocaleString(undefined, { minimumFractionDigits: 0, maximumFractionDigits: 5 })
}

export function formatTimeAgo(utime: number): string {
  const now = Math.floor(Date.now() / 1000)
  const diff = now - utime

  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`

  const date = new Date(utime * 1000)
  const day = date.getDate()
  const month = date.toLocaleString("default", { month: "short" })
  const time = date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false })
  return `${day} ${month}, ${time}`
}

export function formatAddress(
  address: string,
  shorten: boolean = true,
  forceReal: boolean = false,
): string {
  if (!address) return "Unknown"

  let displayAddress = address
  try {
    displayAddress = Address.parse(address).toString({ testOnly: true })
  } catch (_e) {
    // If parsing fails, use original address
  }

  if (!shorten) return displayAddress

  // Standard base64 address or workchain:address format
  if (displayAddress.includes(":")) {
    const [workchain, hash] = displayAddress.split(":")
    return `${workchain}:${hash.slice(0, 6)}…${hash.slice(-6)}`
  }

  if (displayAddress.length > 12) {
    return `${displayAddress.slice(0, 6)}…${displayAddress.slice(-6)}`
  }
  return displayAddress
}

import {Buffer} from "node:buffer"

import {Address} from "@ton/core"

const HEX_HASH_RE = /^[a-fA-F0-9]{64}$/
const BASE64_STD_RE = /^[A-Za-z0-9+/]+={0,2}$/
const BASE64_URL_RE = /^[A-Za-z0-9_-]+$/

export function hashToHex(hash: string): string | undefined {
  const value = hash.trim()
  if (!value) return undefined

  if (HEX_HASH_RE.test(value)) {
    return value.toLowerCase()
  }

  let normalized = value
  if (BASE64_URL_RE.test(normalized)) {
    normalized = normalized.replaceAll("-", "+").replaceAll("_", "/")
  } else if (!BASE64_STD_RE.test(normalized)) {
    return undefined
  }

  const mod = normalized.length % 4
  if (mod === 1) return undefined
  if (mod !== 0) {
    normalized = normalized.padEnd(normalized.length + (4 - mod), "=")
  }

  try {
    const bytes = Buffer.from(normalized, "base64")
    if (bytes.length !== 32) return undefined
    return bytes.toString("hex")
  } catch {
    return undefined
  }
}

export function parseAddress(address: string): Address | undefined {
  if (!address) return undefined
  try {
    return Address.parse(address)
  } catch {
    return undefined
  }
}

export interface AddressFormatOptions {
  readonly testOnly?: boolean
}

const defaultAddressFormat: Required<AddressFormatOptions> = {
  testOnly: true,
}

function getAddressFormatOptions(options?: AddressFormatOptions): Required<AddressFormatOptions> {
  return {
    testOnly: options?.testOnly ?? defaultAddressFormat.testOnly,
  }
}

export function toDisplayAddress(
  address: string,
  options?: AddressFormatOptions,
): string | undefined {
  const parsed = parseAddress(address)
  return parsed ? parsed.toString(getAddressFormatOptions(options)) : undefined
}

export function toTestnetAddress(address: string): string | undefined {
  return toDisplayAddress(address, {testOnly: true})
}

export function normalizeAddress(address: string, options?: AddressFormatOptions): string {
  return toDisplayAddress(address, options) ?? address
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
  return ton.toLocaleString(undefined, {
    minimumFractionDigits: 0,
    maximumFractionDigits: 5,
  })
}

export function formatTimeAgo(utime: number): string {
  const now = Math.floor(Date.now() / 1000)
  const diff = now - utime

  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86_400) return `${Math.floor(diff / 3600)}h ago`

  const date = new Date(utime * 1000)
  const day = date.getDate()
  const month = date.toLocaleString("default", {month: "short"})
  const time = date.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  })
  return `${day} ${month}, ${time}`
}

export function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`
  if (seconds < 86_400) return `${Math.floor(seconds / 3600)}h`
  return `${Math.floor(seconds / 86_400)}d`
}

export function formatAddress(
  address: string,
  shorten: boolean = true,
  options?: AddressFormatOptions,
): string {
  if (!address) return "Unknown"

  let displayAddress = address
  try {
    displayAddress = Address.parse(address).toString(getAddressFormatOptions(options))
  } catch {
    // If parsing fails, use original address
  }

  if (!shorten) return displayAddress

  if (displayAddress.includes(":")) {
    const [workchain, hash] = displayAddress.split(":")
    return `${workchain}:${hash.slice(0, 6)}…${hash.slice(-6)}`
  }

  if (displayAddress.length > 12) {
    return `${displayAddress.slice(0, 6)}…${displayAddress.slice(-6)}`
  }
  return displayAddress
}

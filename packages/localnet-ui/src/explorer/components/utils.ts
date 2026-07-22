import {Buffer} from "node:buffer"

import {Address} from "@ton/core"

const HEX_HASH_RE = /^[a-fA-F0-9]{64}$/
const BASE64_STD_RE = /^[A-Za-z0-9+/]+={0,2}$/
const BASE64_URL_RE = /^[A-Za-z0-9_-]+$/
const TON_DNS_DOMAIN_RE = /^(?:[a-z\d](?:[a-z\d-]{0,61}[a-z\d])?\.)+(?:ton|t\.me)$/i

export function hashToHex(hash: string | null | undefined): string | undefined {
  const value = hash?.trim()
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

export function parseTonDnsSearchQuery(value: string): string | undefined {
  const domain = value.trim().toLowerCase()
  return TON_DNS_DOMAIN_RE.test(domain) ? domain : undefined
}

export interface AddressFormatOptions {
  readonly bounceable?: boolean
  readonly testOnly?: boolean
}

const defaultAddressFormat: Required<AddressFormatOptions> = {
  bounceable: true,
  testOnly: true,
}

function getAddressFormatOptions(options?: AddressFormatOptions): Required<AddressFormatOptions> {
  return {
    bounceable: options?.bounceable ?? defaultAddressFormat.bounceable,
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

export function toRawAddress(address: string): string {
  const parsed = parseAddress(address)
  const rawString = (parsed as {toRawString?: () => string} | undefined)?.toRawString
  return typeof rawString === "function" ? rawString.call(parsed) : address
}

export function isSameAddress(a: string, b: string): boolean {
  if (!a || !b) return false
  const parsedA = parseAddress(a)
  const parsedB = parseAddress(b)
  if (parsedA && parsedB) return parsedA.equals(parsedB)
  return a === b
}

export function formatNano(nano: string | number, maximumFractionDigits = 9): string {
  const n = typeof nano === "string" ? BigInt(nano) : BigInt(nano)
  const ton = Number(n) / 1e9
  return ton.toLocaleString(undefined, {
    minimumFractionDigits: 0,
    maximumFractionDigits,
  })
}

export function formatTimeAgo(
  utime: number,
  nowSeconds: number = Math.floor(Date.now() / 1000),
): string {
  const diff = Math.max(0, nowSeconds - utime)

  if (diff === 0) return "right now"
  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86_400) return `${Math.floor(diff / 3600)}h ago`

  return formatAbsoluteTime(utime, nowSeconds)
}

export function formatRelativeTime(
  utime: number,
  nowSeconds: number = Math.floor(Date.now() / 1000),
): string {
  const diff = Math.max(0, nowSeconds - utime)

  if (diff === 0) return "right now"
  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86_400) return `${Math.floor(diff / 3600)}h ago`
  if (diff < 604_800) return `${Math.floor(diff / 86_400)}d ago`
  if (diff < 2_629_800) return `${Math.floor(diff / 604_800)}w ago`
  if (diff < 31_557_600) return `${Math.floor(diff / 2_629_800)}mo ago`
  return `${Math.floor(diff / 31_557_600)}y ago`
}

export function formatAbsoluteTime(
  utime: number,
  nowSeconds: number = Math.floor(Date.now() / 1000),
): string {
  const date = new Date(utime * 1000)
  const currentYear = new Date(nowSeconds * 1000).getFullYear()
  const day = date.getDate()
  const month = date.toLocaleString("default", {month: "short"})
  const year = date.getFullYear() === currentYear ? "" : ` ${date.getFullYear()}`
  const time = date.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  })
  return `${day} ${month}${year}, ${time}`
}

export function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`
  if (seconds < 86_400) return `${Math.floor(seconds / 3600)}h`
  return `${Math.floor(seconds / 86_400)}d`
}

export function shortenIdentifier(value: string, edgeLength = 6): string {
  return value.length > edgeLength * 2
    ? `${value.slice(0, edgeLength)}…${value.slice(-edgeLength)}`
    : value
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
    return shortenIdentifier(displayAddress)
  }
  return displayAddress
}

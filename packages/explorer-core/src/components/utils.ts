import {Address} from "@ton/core"
import {shortenMiddle} from "@acton/ui"
import {toUnicode} from "punycode/"

import type {AddressInformation} from "../api/types"

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

export function formatDnsName(value: string): string {
  const domain = value.trim()
  try {
    return toUnicode(domain) || domain
  } catch {
    return domain
  }
}

export function mergeAccountDomains(
  primaryDomain: string | undefined,
  domains: readonly string[],
): readonly string[] {
  const uniqueDomains = new Map<string, string>()
  for (const domain of [primaryDomain, ...domains]) {
    const normalizedDomain = domain?.trim()
    const domainKey = normalizedDomain?.toLowerCase()
    if (normalizedDomain && domainKey && !uniqueDomains.has(domainKey)) {
      uniqueDomains.set(domainKey, normalizedDomain)
    }
  }
  return [...uniqueDomains.values()]
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

export function normalizeAddress(address: string, options?: AddressFormatOptions): string {
  return toDisplayAddress(address, options) ?? address
}

export function toAccountQrAddress(
  address: string,
  status: AddressInformation["status"] | undefined,
  options?: AddressFormatOptions,
): string {
  const parsed = parseAddress(address)
  if (!parsed) return address

  return parsed.toString({
    ...getAddressFormatOptions(options),
    bounceable: status === "active" || status === "frozen",
    urlSafe: true,
  })
}

export function toRawAddress(address: string): string {
  const parsed = parseAddress(address)
  if (parsed === undefined) {
    return address
  }
  return parsed.toRawString()
}

export function isSameAddress(a: string, b: string): boolean {
  if (!a || !b) return false
  const parsedA = parseAddress(a)
  const parsedB = parseAddress(b)
  if (parsedA && parsedB) return parsedA.equals(parsedB)
  return a === b
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
    return `${workchain}:${shortenMiddle(hash, {start: 6, end: 6})}`
  }

  if (displayAddress.length > 12) {
    return shortenMiddle(displayAddress, {start: 6, end: 6})
  }
  return displayAddress
}

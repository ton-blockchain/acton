export interface LookupTarget {
  readonly kind: "address" | "code_hash"
  readonly value: string
}

const HEX_CODE_HASH = /^[0-9a-fA-F]{64}$/
const BASE64_CODE_HASH = /^(?:[A-Za-z0-9+/]{43}|[A-Za-z0-9_-]{43})=?$/
const CODE_HASH_BYTES = 32

export function parseLookupTarget(rawValue: string): LookupTarget {
  const value = rawValue.trim()
  if (value.length === 0) {
    throw new Error("Enter a contract address or code hash")
  }

  if (HEX_CODE_HASH.test(value)) {
    return {
      kind: "code_hash",
      value: value.toLowerCase(),
    }
  }

  const base64CodeHash = decodeBase64CodeHash(value)
  if (base64CodeHash) {
    return {
      kind: "code_hash",
      value: base64CodeHash,
    }
  }

  return {
    kind: "address",
    value,
  }
}

function decodeBase64CodeHash(value: string): string | undefined {
  if (!BASE64_CODE_HASH.test(value)) {
    return undefined
  }

  const normalized = value.replaceAll("-", "+").replaceAll("_", "/")
  const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, "=")

  try {
    const decoded = atob(padded)
    if (decoded.length !== CODE_HASH_BYTES) {
      return undefined
    }

    return Array.from(decoded, byte => byte.charCodeAt(0).toString(16).padStart(2, "0")).join("")
  } catch {
    return undefined
  }
}

export function lookupTargetToQuery(target: LookupTarget): string {
  const key = target.kind === "code_hash" ? "code_hash" : "address"
  return `${key}=${encodeURIComponent(target.value)}`
}

export function lookupPath(rawValue: string): string {
  return `/${encodeURIComponent(rawValue.trim())}`
}

export function getPathLookupValue(): string {
  const queryTarget = new URLSearchParams(globalThis.location.search).get("target")
  if (queryTarget?.trim()) {
    return queryTarget.trim()
  }

  const value = globalThis.location.pathname.replace(/^\/+/, "")
  return decodeURIComponent(value)
}

export function shortenMiddle(value: string, left = 10, right = 8): string {
  if (value.length <= left + right + 1) {
    return value
  }
  return `${value.slice(0, left)}…${value.slice(-right)}`
}

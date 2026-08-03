import {Address, Cell} from "@ton/core"

import {loadSuspendedAddressList} from "../cell-inspector/block.tlb.generated"

const ACCOUNT_ID_BITS = 256n
const ACCOUNT_ID_MASK = (1n << ACCOUNT_ID_BITS) - 1n
const SUSPENDED_ACCOUNTS_CACHE_PREFIX = "acton:explorer:suspended-accounts:v1"

export const SUSPENDED_ACCOUNTS_CACHE_TTL_MS = 24 * 60 * 60 * 1000

export interface SuspendedAccountsConfig {
  readonly rawAddresses: readonly string[]
  readonly suspendedUntil: number
}

export type SuspendedAccountsCacheStorage = Pick<Storage, "getItem" | "removeItem" | "setItem">

interface SuspendedAccountsCacheEntry extends SuspendedAccountsConfig {
  readonly expiresAt: number
}

export function parseSuspendedAccountsConfig(configBoc: string): SuspendedAccountsConfig {
  const config = loadSuspendedAddressList(Cell.fromBase64(configBoc).beginParse())
  return {
    rawAddresses: config.addresses.keys().map(suspendedAddressKeyToRawAddress),
    suspendedUntil: config.suspended_until,
  }
}

export function isAddressSuspended(
  config: SuspendedAccountsConfig,
  address: string,
  nowSeconds = Math.floor(Date.now() / 1000),
): boolean {
  if (config.suspendedUntil <= nowSeconds) return false

  try {
    return config.rawAddresses.includes(Address.parse(address).toRawString())
  } catch {
    return false
  }
}

export function readSuspendedAccountsConfigCache(
  apiBaseUrl: string,
  storage: SuspendedAccountsCacheStorage | undefined = getLocalStorage(),
  now = Date.now(),
): SuspendedAccountsConfig | undefined {
  if (!storage) return undefined

  const key = suspendedAccountsCacheKey(apiBaseUrl)
  try {
    const value = storage.getItem(key)
    if (!value) return undefined

    const entry = parseSuspendedAccountsCacheEntry(JSON.parse(value))
    if (!entry || entry.expiresAt <= now) {
      storage.removeItem(key)
      return undefined
    }

    return {
      rawAddresses: entry.rawAddresses,
      suspendedUntil: entry.suspendedUntil,
    }
  } catch {
    try {
      storage.removeItem(key)
    } catch {
      // Storage may be unavailable or read-only.
    }
    return undefined
  }
}

export function writeSuspendedAccountsConfigCache(
  apiBaseUrl: string,
  config: SuspendedAccountsConfig,
  storage: SuspendedAccountsCacheStorage | undefined = getLocalStorage(),
  now = Date.now(),
): void {
  if (!storage) return

  try {
    storage.setItem(
      suspendedAccountsCacheKey(apiBaseUrl),
      JSON.stringify({
        rawAddresses: config.rawAddresses,
        suspendedUntil: config.suspendedUntil,
        expiresAt: now + SUSPENDED_ACCOUNTS_CACHE_TTL_MS,
      } satisfies SuspendedAccountsCacheEntry),
    )
  } catch {
    // A cache write must not make the API request fail.
  }
}

function suspendedAddressKeyToRawAddress(key: bigint): string {
  const workchain = Number(BigInt.asIntN(32, key >> ACCOUNT_ID_BITS))
  const accountId = Buffer.from(
    (key & ACCOUNT_ID_MASK).toString(16).padStart(Number(ACCOUNT_ID_BITS / 4n), "0"),
    "hex",
  )
  return new Address(workchain, accountId).toRawString()
}

function suspendedAccountsCacheKey(apiBaseUrl: string): string {
  return `${SUSPENDED_ACCOUNTS_CACHE_PREFIX}:${encodeURIComponent(apiBaseUrl)}`
}

function parseSuspendedAccountsCacheEntry(value: unknown): SuspendedAccountsCacheEntry | undefined {
  if (!isRecord(value)) return undefined

  const {expiresAt, rawAddresses, suspendedUntil} = value
  if (
    typeof expiresAt !== "number" ||
    !Number.isSafeInteger(expiresAt) ||
    expiresAt <= 0 ||
    !Array.isArray(rawAddresses) ||
    !rawAddresses.every(isRawAddress) ||
    typeof suspendedUntil !== "number" ||
    !Number.isSafeInteger(suspendedUntil) ||
    suspendedUntil < 0
  ) {
    return undefined
  }

  return {
    expiresAt,
    rawAddresses,
    suspendedUntil,
  }
}

function isRawAddress(value: unknown): value is string {
  if (typeof value !== "string") return false
  try {
    Address.parseRaw(value)
    return true
  } catch {
    return false
  }
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null
}

function getLocalStorage(): Storage | undefined {
  try {
    return globalThis.localStorage
  } catch {
    return undefined
  }
}

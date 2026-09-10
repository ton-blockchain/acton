export const FAUCET_REQUEST_HISTORY_STORAGE_KEY = "actonscanFaucetRequestHistory"
export const FAUCET_REQUEST_LIMIT = 2

export const FAUCET_REQUEST_WINDOW_MS = 60 * 60 * 1000
const MAX_STORED_REQUEST_TIMESTAMPS = 1024

export interface FaucetUsage {
  readonly used: number
  readonly limitReached: boolean
  readonly lastRequestAt?: number
  readonly availableAgainAt?: number
  readonly refreshAt?: number
}

export function readFaucetUsage(
  now = Date.now(),
  limit = FAUCET_REQUEST_LIMIT,
  windowMs = FAUCET_REQUEST_WINDOW_MS,
): FaucetUsage {
  return usageFromTimestamps(readActiveTimestamps(now, windowMs), limit, windowMs)
}

export function recordFaucetRequest(
  now = Date.now(),
  limit = FAUCET_REQUEST_LIMIT,
  windowMs = FAUCET_REQUEST_WINDOW_MS,
): FaucetUsage {
  const storedTimestamps = readStoredTimestamps(now)
  storedTimestamps.push(now)
  writeTimestamps(storedTimestamps.slice(-MAX_STORED_REQUEST_TIMESTAMPS))
  return usageFromTimestamps(activeTimestamps(storedTimestamps, now, windowMs), limit, windowMs)
}

function readActiveTimestamps(now: number, windowMs: number): number[] {
  return activeTimestamps(readStoredTimestamps(now), now, windowMs)
}

function readStoredTimestamps(now: number): number[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(FAUCET_REQUEST_HISTORY_STORAGE_KEY) ?? "[]")
    if (!Array.isArray(parsed)) return []

    return parsed
      .filter(
        (timestamp): timestamp is number => Number.isSafeInteger(timestamp) && timestamp <= now,
      )
      .sort((left, right) => left - right)
  } catch {
    return []
  }
}

function activeTimestamps(timestamps: readonly number[], now: number, windowMs: number): number[] {
  const cutoff = now - windowMs
  return timestamps.filter(timestamp => timestamp > cutoff)
}

function writeTimestamps(timestamps: readonly number[]): void {
  try {
    localStorage.setItem(FAUCET_REQUEST_HISTORY_STORAGE_KEY, JSON.stringify(timestamps))
  } catch {
    // The faucet still works when browser storage is unavailable
  }
}

function usageFromTimestamps(
  timestamps: readonly number[],
  limit: number,
  windowMs: number,
): FaucetUsage {
  const used = Math.min(timestamps.length, limit)
  const limitReached = timestamps.length >= limit
  const availableAgainIndex = timestamps.length - limit
  const availableAgainAt = limitReached ? timestamps[availableAgainIndex] + windowMs : undefined

  return {
    used,
    limitReached,
    lastRequestAt: timestamps.at(-1),
    availableAgainAt,
    refreshAt: timestamps.length > 0 ? timestamps[0] + windowMs : undefined,
  }
}

export const FAUCET_REQUEST_HISTORY_STORAGE_KEY = "actonscanFaucetRequestHistory"
export const FAUCET_REQUEST_LIMIT = 2

const FAUCET_REQUEST_WINDOW_MS = 60 * 60 * 1000

export interface FaucetUsage {
  readonly used: number
  readonly limitReached: boolean
  readonly lastRequestAt?: number
  readonly availableAgainAt?: number
  readonly refreshAt?: number
}

export function readFaucetUsage(now = Date.now()): FaucetUsage {
  return usageFromTimestamps(readActiveTimestamps(now))
}

export function recordFaucetRequest(now = Date.now()): FaucetUsage {
  const timestamps = readActiveTimestamps(now)
  timestamps.push(now)
  writeTimestamps(timestamps)
  return usageFromTimestamps(timestamps)
}

function readActiveTimestamps(now: number): number[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(FAUCET_REQUEST_HISTORY_STORAGE_KEY) ?? "[]")
    if (!Array.isArray(parsed)) {
      writeTimestamps([])
      return []
    }

    const cutoff = now - FAUCET_REQUEST_WINDOW_MS
    const timestamps = parsed
      .filter(
        (timestamp): timestamp is number =>
          Number.isSafeInteger(timestamp) && timestamp > cutoff && timestamp <= now,
      )
      .sort((left, right) => left - right)

    if (
      timestamps.length !== parsed.length ||
      timestamps.some((timestamp, index) => timestamp !== parsed[index])
    ) {
      writeTimestamps(timestamps)
    }
    return timestamps
  } catch {
    writeTimestamps([])
    return []
  }
}

function writeTimestamps(timestamps: readonly number[]): void {
  try {
    localStorage.setItem(FAUCET_REQUEST_HISTORY_STORAGE_KEY, JSON.stringify(timestamps))
  } catch {
    // The faucet still works when browser storage is unavailable
  }
}

function usageFromTimestamps(timestamps: readonly number[]): FaucetUsage {
  const used = Math.min(timestamps.length, FAUCET_REQUEST_LIMIT)
  const limitReached = timestamps.length >= FAUCET_REQUEST_LIMIT
  const availableAgainIndex = timestamps.length - FAUCET_REQUEST_LIMIT
  const availableAgainAt = limitReached
    ? timestamps[availableAgainIndex] + FAUCET_REQUEST_WINDOW_MS
    : undefined

  return {
    used,
    limitReached,
    lastRequestAt: timestamps.at(-1),
    availableAgainAt,
    refreshAt: timestamps.length > 0 ? timestamps[0] + FAUCET_REQUEST_WINDOW_MS : undefined,
  }
}

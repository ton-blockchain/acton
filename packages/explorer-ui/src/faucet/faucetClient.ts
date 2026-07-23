const DEFAULT_FAUCET_URL = "https://faucet.acton.monster/"
const DEFAULT_MAX_SOLVE_TTL_SECONDS = 60
const DEFAULT_MAX_NONCE_ATTEMPTS = 1_000_000_000
const FAUCET_CLIENT_HEADER = "actonscan/1.0.0"
const FAUCET_DEVICE_UID_STORAGE_KEY = "actonscanFaucetDeviceUid"

export interface FaucetChallenge {
  readonly version: number
  readonly challenge: string
  readonly difficulty: number
  readonly maxSolveTtlSeconds: number
  readonly maxNonceAttempts: number
}

export interface FaucetClaim {
  readonly message: string
}

export class FaucetRequestError extends Error {
  readonly status: number

  constructor(message: string, status: number) {
    super(message)
    this.name = "FaucetRequestError"
    this.status = status
  }
}

interface FaucetChallengeResponse {
  readonly version?: unknown
  readonly challenge?: unknown
  readonly difficulty?: unknown
  readonly max_solve_ttl_seconds?: unknown
  readonly max_nonce_attempts?: unknown
}

interface FaucetMessageResponse {
  readonly error?: unknown
  readonly message?: unknown
}

export async function requestFaucetChallenge(
  address: string,
  signal?: AbortSignal,
): Promise<FaucetChallenge> {
  const payload = await faucetRequest<FaucetChallengeResponse>(
    "challenge",
    {address, type: 1},
    signal,
  )
  const maxSolveTtlSeconds =
    payload.max_solve_ttl_seconds === undefined
      ? DEFAULT_MAX_SOLVE_TTL_SECONDS
      : positiveSafeInteger(payload.max_solve_ttl_seconds, "PoW solve time limit")
  const maxNonceAttempts =
    payload.max_nonce_attempts === undefined
      ? DEFAULT_MAX_NONCE_ATTEMPTS
      : positiveSafeInteger(payload.max_nonce_attempts, "PoW nonce limit")

  if (payload.version !== 1) {
    throw new Error(`Unsupported faucet challenge version: ${String(payload.version)}`)
  }
  if (typeof payload.challenge !== "string" || payload.challenge.length === 0) {
    throw new Error("Faucet returned an invalid PoW challenge")
  }
  if (
    typeof payload.difficulty !== "number" ||
    !Number.isInteger(payload.difficulty) ||
    payload.difficulty < 0 ||
    payload.difficulty > 256
  ) {
    throw new Error(`Faucet returned an invalid PoW difficulty: ${String(payload.difficulty)}`)
  }

  return {
    version: payload.version,
    challenge: payload.challenge,
    difficulty: payload.difficulty,
    maxSolveTtlSeconds,
    maxNonceAttempts,
  }
}

export async function submitFaucetClaim(
  address: string,
  challenge: FaucetChallenge,
  nonce: number,
  signal?: AbortSignal,
): Promise<FaucetClaim> {
  const payload = await faucetRequest<FaucetMessageResponse>(
    "claim",
    {
      address,
      version: challenge.version,
      challenge: challenge.challenge,
      nonce,
      type: 1,
    },
    signal,
  )

  return {
    message:
      typeof payload.message === "string" && payload.message.trim()
        ? payload.message
        : "Your testnet claim has been queued",
  }
}

async function faucetRequest<T>(
  path: string,
  payload: Record<string, unknown>,
  signal?: AbortSignal,
): Promise<T> {
  const response = await fetch(new URL(path, faucetBaseUrl()), {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-acton-client": FAUCET_CLIENT_HEADER,
      "x-device-uid": faucetDeviceUid(),
    },
    body: JSON.stringify(payload),
    signal,
  })
  const text = await response.text()
  const parsed = parseJson(text)

  if (!response.ok) {
    throw new FaucetRequestError(faucetErrorMessage(parsed, text, response.status), response.status)
  }
  if (!isRecord(parsed)) {
    throw new Error("Faucet returned an invalid JSON response")
  }

  return parsed as T
}

function faucetBaseUrl(): string {
  const configured = import.meta.env.VITE_FAUCET_URL?.trim()
  const value = configured || DEFAULT_FAUCET_URL
  return value.endsWith("/") ? value : `${value}/`
}

function faucetDeviceUid(): string {
  try {
    const stored = localStorage.getItem(FAUCET_DEVICE_UID_STORAGE_KEY)
    if (stored && isValidDeviceUid(stored)) {
      return stored
    }

    const generated = generateDeviceUid()
    localStorage.setItem(FAUCET_DEVICE_UID_STORAGE_KEY, generated)
    return generated
  } catch {
    return "default"
  }
}

function generateDeviceUid(): string {
  if (typeof crypto.randomUUID === "function") {
    return crypto.randomUUID()
  }

  const bytes = crypto.getRandomValues(new Uint8Array(16))
  return Array.from(bytes, byte => byte.toString(16).padStart(2, "0")).join("")
}

function isValidDeviceUid(value: string): boolean {
  return value === "default" || value.length === 32 || value.length === 36
}

function positiveSafeInteger(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`Faucet returned an invalid ${label}: ${String(value)}`)
  }
  return value
}

function parseJson(value: string): unknown {
  if (!value) return undefined

  try {
    return JSON.parse(value)
  } catch {
    return undefined
  }
}

function faucetErrorMessage(parsed: unknown, raw: string, status: number): string {
  if (isRecord(parsed)) {
    if (typeof parsed.error === "string" && parsed.error.trim()) return parsed.error
    if (typeof parsed.message === "string" && parsed.message.trim()) return parsed.message
  }
  if (raw.trim()) return raw.trim()
  return `Faucet request failed with status ${status}`
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

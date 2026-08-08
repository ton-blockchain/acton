const DEFAULT_FAUCET_URL = "https://faucet.acton.monster/"
const DEFAULT_MAX_SOLVE_TTL_SECONDS = 60
const DEFAULT_MAX_NONCE_ATTEMPTS = 1_000_000_000
const FAUCET_CLIENT_HEADER = "actonscan/1.0.0"
const FAUCET_DEVICE_UID_STORAGE_KEY = "actonscanFaucetDeviceUid"
const FAUCET_SESSION_STORAGE_KEY = "actonscanFaucetSession"

export type FaucetTier = "guest" | "verified" | "established"

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

export interface FaucetAuthStatus {
  readonly enabled: boolean
  readonly guestMaxRequests: number
  readonly verifiedMaxRequests: number
  readonly establishedMaxRequests: number
  readonly windowSeconds: number
}

export interface FaucetSession {
  readonly githubUserId: number
  readonly login: string
  readonly tier: FaucetTier
  readonly maxRequests: number
  readonly accountAgeDays: number
  readonly publicRepos: number
  readonly followers: number
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

interface FaucetAuthStatusResponse {
  readonly enabled?: unknown
  readonly guestMaxRequests?: unknown
  readonly verifiedMaxRequests?: unknown
  readonly establishedMaxRequests?: unknown
  readonly windowSeconds?: unknown
}

interface FaucetSessionResponse {
  readonly authenticated?: unknown
  readonly githubUserId?: unknown
  readonly login?: unknown
  readonly tier?: unknown
  readonly maxRequests?: unknown
  readonly accountAgeDays?: unknown
  readonly publicRepos?: unknown
  readonly followers?: unknown
  readonly token?: unknown
}

export async function requestFaucetAuthStatus(signal?: AbortSignal): Promise<FaucetAuthStatus> {
  const payload = await faucetGet<FaucetAuthStatusResponse>("auth/status", signal, false)

  return {
    enabled: payload.enabled === true,
    guestMaxRequests: positiveSafeInteger(payload.guestMaxRequests, "guest request limit"),
    verifiedMaxRequests: positiveSafeInteger(payload.verifiedMaxRequests, "verified request limit"),
    establishedMaxRequests: positiveSafeInteger(
      payload.establishedMaxRequests,
      "established request limit",
    ),
    windowSeconds: positiveSafeInteger(payload.windowSeconds, "request window"),
  }
}

export function githubAuthorizationUrl(): string {
  const url = new URL("auth/github/start", faucetBaseUrl())
  url.searchParams.set("device_uid", faucetDeviceUid())
  return url.toString()
}

export async function exchangeGitHubGrant(
  grant: string,
  signal?: AbortSignal,
): Promise<FaucetSession> {
  const payload = await faucetRequest<FaucetSessionResponse>(
    "auth/exchange",
    {grant},
    signal,
    false,
  )
  if (typeof payload.token !== "string" || payload.token.length < 32) {
    throw new Error("Faucet returned an invalid GitHub session token")
  }
  const session = parseFaucetSession(payload)
  writeFaucetSessionToken(payload.token)
  return session
}

export async function requestFaucetSession(
  signal?: AbortSignal,
): Promise<FaucetSession | undefined> {
  if (!readFaucetSessionToken()) return undefined

  try {
    const payload = await faucetGet<FaucetSessionResponse>("auth/session", signal)
    return parseFaucetSession(payload)
  } catch (error) {
    if (error instanceof FaucetRequestError && error.status === 401) {
      clearFaucetSession()
      return undefined
    }
    throw error
  }
}

export async function disconnectFaucetSession(signal?: AbortSignal): Promise<void> {
  try {
    if (readFaucetSessionToken()) {
      await faucetFetch("auth/session", {method: "DELETE", signal})
    }
  } catch (error) {
    if (!(error instanceof FaucetRequestError && error.status === 401)) {
      throw error
    }
  }

  // A successful delete and a confirmed unauthorized response both mean there
  // is no usable server-side session left. Transient failures keep the token.
  clearFaucetSession()
}

export function clearFaucetSession(): void {
  try {
    sessionStorage.removeItem(FAUCET_SESSION_STORAGE_KEY)
  } catch {
    // The page remains usable as a guest when browser storage is unavailable
  }
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

function faucetRequest<T>(
  path: string,
  payload: Record<string, unknown>,
  signal?: AbortSignal,
  authorized = true,
): Promise<T> {
  return faucetFetch<T>(path, {
    method: "POST",
    body: JSON.stringify(payload),
    signal,
    authorized,
    contentType: true,
  })
}

function faucetGet<T>(path: string, signal?: AbortSignal, authorized = true): Promise<T> {
  return faucetFetch<T>(path, {method: "GET", signal, authorized})
}

interface FaucetFetchOptions {
  readonly method: "GET" | "POST" | "DELETE"
  readonly body?: string
  readonly signal?: AbortSignal
  readonly authorized?: boolean
  readonly contentType?: boolean
}

async function faucetFetch<T = unknown>(path: string, options: FaucetFetchOptions): Promise<T> {
  const headers: Record<string, string> = {
    "x-acton-client": FAUCET_CLIENT_HEADER,
    "x-device-uid": faucetDeviceUid(),
  }
  if (options.contentType) headers["content-type"] = "application/json"
  if (options.authorized !== false) {
    const sessionToken = readFaucetSessionToken()
    if (sessionToken) headers.authorization = `Bearer ${sessionToken}`
  }

  const response = await fetch(new URL(path, faucetBaseUrl()), {
    method: options.method,
    headers,
    body: options.body,
    signal: options.signal,
  })
  const text = await response.text()
  const parsed = parseJson(text)

  if (!response.ok) {
    if (response.status === 401 && options.authorized !== false) clearFaucetSession()
    throw new FaucetRequestError(faucetErrorMessage(parsed, text, response.status), response.status)
  }
  if (response.status === 204) return undefined as T
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

function readFaucetSessionToken(): string | undefined {
  try {
    const token = sessionStorage.getItem(FAUCET_SESSION_STORAGE_KEY)?.trim()
    return token || undefined
  } catch {
    return undefined
  }
}

function writeFaucetSessionToken(token: string): void {
  try {
    sessionStorage.setItem(FAUCET_SESSION_STORAGE_KEY, token)
  } catch {
    // Requests continue as guest when browser storage is unavailable
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

function nonNegativeSafeInteger(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`Faucet returned an invalid ${label}: ${String(value)}`)
  }
  return value
}

function parseFaucetSession(payload: FaucetSessionResponse): FaucetSession {
  if (
    payload.authenticated !== true ||
    typeof payload.login !== "string" ||
    payload.login.length === 0 ||
    (payload.tier !== "guest" && payload.tier !== "verified" && payload.tier !== "established")
  ) {
    throw new Error("Faucet returned an invalid GitHub session")
  }

  return {
    githubUserId: positiveSafeInteger(payload.githubUserId, "GitHub user ID"),
    login: payload.login,
    tier: payload.tier,
    maxRequests: positiveSafeInteger(payload.maxRequests, "request limit"),
    accountAgeDays: nonNegativeSafeInteger(payload.accountAgeDays, "account age"),
    publicRepos: nonNegativeSafeInteger(payload.publicRepos, "public repository count"),
    followers: nonNegativeSafeInteger(payload.followers, "follower count"),
  }
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

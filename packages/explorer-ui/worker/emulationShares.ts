import {parseSharedEmulation, type SharedEmulation} from "@acton/explorer-core/pages/emulateSharing"

export const EMULATION_SHARE_TTL_MS = 30 * 24 * 60 * 60 * 1000

const MAX_REQUEST_BYTES = 1024 * 1024
const SHARE_ID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i
const OBJECT_PREFIX = "emulations/"

interface R2ObjectBody {
  text(): Promise<string>
}

export interface EmulationShareBucket {
  get(key: string): Promise<R2ObjectBody | null>
  put(
    key: string,
    value: string,
    options?: {
      readonly httpMetadata?: {readonly contentType?: string}
      readonly customMetadata?: Record<string, string>
    },
  ): Promise<unknown>
  delete(key: string): Promise<void>
}

export interface EmulationSharePagesContext {
  readonly request: Request
  readonly env: {
    readonly EMULATION_SHARES?: EmulationShareBucket
  }
  readonly params?: {
    readonly id?: string | readonly string[]
  }
  waitUntil?(promise: Promise<unknown>): void
}

interface StoredEmulationShare {
  readonly expiresAt: number
  readonly emulation: SharedEmulation
}

export async function createEmulationShareResponse(
  context: EmulationSharePagesContext,
  now = Date.now(),
): Promise<Response> {
  if (context.request.method !== "POST") {
    return jsonError(405, "Method not allowed", {allow: "POST"})
  }

  const bucket = context.env.EMULATION_SHARES
  if (!bucket) {
    return jsonError(503, "Emulation sharing is not configured")
  }

  const contentType = context.request.headers.get("content-type")?.toLowerCase() ?? ""
  if (!contentType.startsWith("application/json")) {
    return jsonError(415, "Content-Type must be application/json")
  }

  const contentLength = Number(context.request.headers.get("content-length"))
  if (Number.isFinite(contentLength) && contentLength > MAX_REQUEST_BYTES) {
    return jsonError(413, "Emulation share is too large")
  }

  let body: string
  try {
    body = await context.request.text()
  } catch {
    return jsonError(400, "Failed to read request body")
  }
  if (new TextEncoder().encode(body).byteLength > MAX_REQUEST_BYTES) {
    return jsonError(413, "Emulation share is too large")
  }

  let parsed: unknown
  try {
    parsed = JSON.parse(body)
  } catch {
    return jsonError(400, "Request body must be valid JSON")
  }
  const emulation = parseSharedEmulation(parsed)
  if (!emulation) {
    return jsonError(400, "Request body is not a valid emulation")
  }

  const id = crypto.randomUUID()
  const expiresAt = now + EMULATION_SHARE_TTL_MS
  const stored: StoredEmulationShare = {expiresAt, emulation}
  try {
    await bucket.put(objectKey(id), JSON.stringify(stored), {
      httpMetadata: {contentType: "application/json; charset=utf-8"},
      customMetadata: {
        createdAt: new Date(now).toISOString(),
        expiresAt: new Date(expiresAt).toISOString(),
      },
    })
  } catch {
    return jsonError(503, "Failed to store emulation share")
  }

  return jsonResponse({id, expiresAt}, 201)
}

export async function readEmulationShareResponse(
  context: EmulationSharePagesContext,
  now = Date.now(),
): Promise<Response> {
  if (context.request.method !== "GET") {
    return jsonError(405, "Method not allowed", {allow: "GET"})
  }

  const bucket = context.env.EMULATION_SHARES
  if (!bucket) {
    return jsonError(503, "Emulation sharing is not configured")
  }

  const rawId = context.params?.id
  const id = Array.isArray(rawId) ? rawId[0] : rawId
  if (!id || !SHARE_ID_PATTERN.test(id)) {
    return jsonError(404, "Emulation share not found")
  }

  let object: R2ObjectBody | null
  try {
    object = await bucket.get(objectKey(id))
  } catch {
    return jsonError(503, "Failed to load emulation share")
  }
  if (!object) {
    return jsonError(404, "Emulation share not found")
  }

  let stored: StoredEmulationShare | undefined
  try {
    stored = parseStoredEmulationShare(JSON.parse(await object.text()))
  } catch {
    stored = undefined
  }
  if (!stored) {
    return jsonError(500, "Stored emulation share is invalid")
  }
  if (stored.expiresAt <= now) {
    const deletion = bucket.delete(objectKey(id)).catch(() => undefined)
    if (context.waitUntil) {
      context.waitUntil(deletion)
    } else {
      await deletion
    }
    return jsonError(410, "Emulation share has expired")
  }

  return jsonResponse({emulation: stored.emulation, expiresAt: stored.expiresAt})
}

function parseStoredEmulationShare(value: unknown): StoredEmulationShare | undefined {
  if (!isRecord(value) || !isTimestamp(value.expiresAt)) {
    return undefined
  }
  const emulation = parseSharedEmulation(value.emulation)
  return emulation ? {expiresAt: value.expiresAt, emulation} : undefined
}

function objectKey(id: string): string {
  return `${OBJECT_PREFIX}${id}.json`
}

function jsonError(status: number, error: string, headers?: HeadersInit): Response {
  return jsonResponse({error}, status, headers)
}

function jsonResponse(payload: unknown, status = 200, headers?: HeadersInit): Response {
  const responseHeaders = new Headers(headers)
  responseHeaders.set("cache-control", "no-store")
  responseHeaders.set("content-type", "application/json; charset=utf-8")
  responseHeaders.set("x-content-type-options", "nosniff")
  return new Response(JSON.stringify(payload), {status, headers: responseHeaders})
}

function isTimestamp(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value)
}

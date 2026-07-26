export const STUDIO_API_VERSION = 1

export interface StudioInfo {
  readonly protocolVersion: number
  readonly serverVersion: string
  readonly workspace?: {
    readonly name: string
    readonly walletNames?: readonly string[]
  }
}

export type StudioConnectionState = "connecting" | "connected" | "disconnected"

export type EnvironmentStatus = "starting" | "running" | "stopping" | "stopped" | "failed"

export interface EnvironmentConfig {
  readonly port: number
  readonly forkNetwork?: string
  readonly forkBlockNumber?: number
  readonly accounts: readonly string[]
  readonly rateLimit?: number
  readonly responseDelayMs?: number
  readonly blockIntervalMs?: number
  readonly noMining: boolean
  readonly mineEmptyBlocks: boolean
}

export interface StudioEnvironment {
  readonly id: string
  readonly name: string
  readonly status: EnvironmentStatus
  readonly rpcUrl: string
  readonly config: EnvironmentConfig
  readonly error?: string
}

export interface CreateEnvironmentRequest {
  readonly name: string
  readonly port?: number
  readonly forkNetwork?: string
  readonly forkBlockNumber?: number
  readonly accounts: readonly string[]
  readonly rateLimit?: number
  readonly responseDelayMs?: number
  readonly blockIntervalMs?: number
  readonly noMining: boolean
  readonly mineEmptyBlocks: boolean
}

export async function fetchStudioInfo(signal: AbortSignal): Promise<StudioInfo> {
  const info = await requestJson<StudioInfo>("/api/v1/info", {
    headers: {accept: "application/json"},
    signal,
  })

  if (info.protocolVersion !== STUDIO_API_VERSION) {
    throw new Error(`Unsupported Studio API version ${info.protocolVersion}`)
  }

  return info
}

export function fetchStudioEnvironments(signal?: AbortSignal): Promise<StudioEnvironment[]> {
  return requestJson<StudioEnvironment[]>("/api/v1/environments", {
    headers: {accept: "application/json"},
    signal,
  })
}

export function createStudioEnvironment(
  request: CreateEnvironmentRequest,
): Promise<StudioEnvironment> {
  return requestJson<StudioEnvironment>("/api/v1/environments", {
    method: "POST",
    headers: {
      accept: "application/json",
      "content-type": "application/json",
    },
    body: JSON.stringify(request),
  })
}

export function stopStudioEnvironment(environmentId: string): Promise<StudioEnvironment> {
  return requestJson<StudioEnvironment>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/stop`,
    {
      method: "POST",
      headers: {accept: "application/json"},
    },
  )
}

export function restartStudioEnvironment(environmentId: string): Promise<StudioEnvironment> {
  return requestJson<StudioEnvironment>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/restart`,
    {
      method: "POST",
      headers: {accept: "application/json"},
    },
  )
}

async function requestJson<T>(input: string, init?: RequestInit): Promise<T> {
  const response = await fetch(input, init)
  if (response.ok) {
    return (await response.json()) as T
  }

  const fallbackMessage = `Studio server returned ${response.status}`
  let body: {readonly error?: {readonly message?: string}}
  try {
    body = (await response.json()) as typeof body
  } catch (error) {
    throw new Error(fallbackMessage, {cause: error})
  }
  throw new Error(body.error?.message || fallbackMessage)
}

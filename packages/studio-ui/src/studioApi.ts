export const STUDIO_API_VERSION = 1

export interface StudioInfo {
  readonly protocolVersion: number
  readonly serverVersion: string
  readonly workspace?: {
    readonly name: string
  }
}

export type StudioConnectionState = "connecting" | "connected" | "disconnected"

export async function fetchStudioInfo(signal: AbortSignal): Promise<StudioInfo> {
  const response = await fetch("/api/v1/info", {
    headers: {accept: "application/json"},
    signal,
  })

  if (!response.ok) {
    throw new Error(`Studio server returned ${response.status}`)
  }

  const info = (await response.json()) as StudioInfo
  if (info.protocolVersion !== STUDIO_API_VERSION) {
    throw new Error(`Unsupported Studio API version ${info.protocolVersion}`)
  }

  return info
}

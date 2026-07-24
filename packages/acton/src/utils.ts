import {LocalnetApiError, errorMessage} from "./errors.js"

const TRAILING_SLASHES = /\/+$/

export function delay(ms: number): Promise<void> {
  return new Promise(resolve => {
    setTimeout(resolve, ms)
  })
}

export function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null
}

export function normalizeEndpoint(endpoint: string): string {
  return endpoint.replace(TRAILING_SLASHES, "")
}

export function parseJson<T>(text: string): T {
  try {
    return JSON.parse(text) as T
  } catch (error) {
    throw new LocalnetApiError(`Localnet returned invalid JSON: ${errorMessage(error)}`, {
      cause: error,
    })
  }
}

export function toEndpointUrl(endpoint: string, path: string): string {
  return new URL(path, `${endpoint}/`).toString()
}

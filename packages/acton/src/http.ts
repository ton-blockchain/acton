import {LocalnetApiError, errorMessage} from "./errors.js"
import type {ApiEnvelope} from "./types.js"
import {isRecord, parseJson, toEndpointUrl} from "./utils.js"

export class LocalnetHttpClient {
  constructor(
    private readonly endpoint: string,
    private readonly authToken?: string,
  ) {}

  async getJson<T>(path: string): Promise<T> {
    return this.requestJson<T>(path, {headers: this.headers(), method: "GET"})
  }

  async postJson<T>(path: string, body: unknown): Promise<T> {
    return this.requestJson<T>(path, {
      body: JSON.stringify(body),
      headers: this.headers({"content-type": "application/json"}),
      method: "POST",
    })
  }

  async postRawJson<T>(path: string, body: unknown): Promise<T> {
    const response = await fetch(toEndpointUrl(this.endpoint, path), {
      body: JSON.stringify(body),
      headers: this.headers({"content-type": "application/json"}),
      method: "POST",
    })
    const text = await response.text()
    const payload = parseJson<T>(text)

    if (!response.ok) {
      const message =
        isRecord(payload) && typeof payload.error === "string"
          ? payload.error
          : `Localnet request failed with HTTP ${response.status}`
      throw new LocalnetApiError(message, {status: response.status})
    }

    return payload
  }

  private async requestJson<T>(path: string, init: RequestInit): Promise<T> {
    const response = await fetch(toEndpointUrl(this.endpoint, path), init)
    const text = await response.text()
    const payload = parseJson<ApiEnvelope<T>>(text)

    if (!response.ok) {
      throw new LocalnetApiError(`Localnet request failed with HTTP ${response.status}`, {
        status: response.status,
      })
    }

    if (!payload.ok) {
      throw new LocalnetApiError(payload.error ?? "Localnet request failed", {
        code: payload.code,
        status: response.status,
      })
    }

    if (!("result" in payload)) {
      throw new LocalnetApiError("Localnet response does not contain result")
    }

    return payload.result as T
  }

  private headers(headers: Record<string, string> = {}): Record<string, string> {
    if (!this.authToken) {
      return headers
    }
    return {
      ...headers,
      Authorization: `Bearer ${this.authToken}`,
    }
  }
}

export function describeRequestError(error: unknown): string {
  return errorMessage(error)
}

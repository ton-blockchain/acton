import type { ApiResponse, FullAccountState, Transaction, V3TracesResponse } from "./types"

interface TonClientOptions {
  readonly v2BaseUrl: string
  readonly v3BaseUrl: string
  readonly addressNameBaseUrl: string
}

export class TonClient {
  private readonly v2BaseUrl: string
  private readonly v3BaseUrl: string
  private readonly addressNameBaseUrl: string

  constructor({ v2BaseUrl, v3BaseUrl, addressNameBaseUrl }: TonClientOptions) {
    this.v2BaseUrl = v2BaseUrl
    this.v3BaseUrl = v3BaseUrl
    this.addressNameBaseUrl = addressNameBaseUrl
  }

  async getAddressInformation(address: string, seqno?: number): Promise<FullAccountState> {
    const url = this.buildUrl(this.v2BaseUrl, "/getAddressInformation")
    url.searchParams.append("address", address)
    if (seqno !== undefined) {
      url.searchParams.append("seqno", seqno.toString())
    }
    return this.request(url, "Failed to fetch address information")
  }

  async getTransactions(address: string, limit = 20): Promise<Transaction[]> {
    const url = this.buildUrl(this.v2BaseUrl, "/getTransactions")
    url.searchParams.append("address", address)
    url.searchParams.append("limit", limit.toString())
    return this.request(url, "Failed to fetch transactions")
  }

  async getTraces(hash: string): Promise<V3TracesResponse> {
    const url = this.buildUrl(this.v3BaseUrl, "/traces")
    url.searchParams.append("hash", hash)
    return this.request(url, "Failed to fetch traces")
  }

  async getAddressName(address: string): Promise<string | null> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/address-name")
    url.searchParams.append("address", address)
    return this.request(url, "Failed to fetch address name")
  }

  async setAddressName(address: string, name: string): Promise<void> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/address-name")
    await this.request<null>(url, "Failed to set address name", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ address, name }),
    })
  }

  private buildUrl(base: string, path: string): URL {
    const fullBase = base.startsWith("http") ? base : `${window.location.origin}${base}`
    return new URL(`${fullBase}${path}`)
  }

  private async request<T>(url: URL, errorMessage: string, options?: RequestInit): Promise<T> {
    const response = await fetch(url.toString(), options)
    const data = (await response.json()) as ApiResponse<T>
    if (!data.ok) throw new Error(data.error || errorMessage)
    return data.result
  }
}

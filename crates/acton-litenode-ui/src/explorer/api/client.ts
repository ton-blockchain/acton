import type {
  ApiResponse,
  FullAccountState,
  JettonMaster,
  JettonWallet,
  Transaction,
  V3TracesResponse,
} from "./types"

interface TonClientOptions {
  readonly v2BaseUrl: string
  readonly v3BaseUrl: string
  readonly addressNameBaseUrl: string
}

export class TonClient {
  private readonly v2BaseUrl: string
  private readonly v3BaseUrl: string
  private readonly addressNameBaseUrl: string

  constructor({v2BaseUrl, v3BaseUrl, addressNameBaseUrl}: TonClientOptions) {
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

  async getJettonMasters(address?: string[], limit = 100, offset = 0): Promise<JettonMaster[]> {
    if (address && address.length > 0) {
      const results = await Promise.all(
        address.map(async addr => {
          const singleUrl = this.buildUrl(this.v3BaseUrl, "/jetton/masters")
          singleUrl.searchParams.append("address", addr)
          try {
            const response = await this.request<{jetton_masters: JettonMaster[]}>(
              singleUrl,
              "Failed to fetch jetton master",
            )
            return response.jetton_masters
          } catch (error) {
            console.error(`Failed to fetch jetton master for ${addr}`, error)
            return []
          }
        }),
      )
      return results.flat()
    }

    const url = this.buildUrl(this.v3BaseUrl, "/jetton/masters")
    url.searchParams.append("limit", limit.toString())
    url.searchParams.append("offset", offset.toString())
    const response = await this.request<{jetton_masters: JettonMaster[]}>(
      url,
      "Failed to fetch jetton masters",
    )
    return response.jetton_masters
  }

  async getJettonWallets(
    owner_address?: string[],
    jetton_address?: string[],
  ): Promise<JettonWallet[]> {
    if (
      (!owner_address || owner_address.length === 0) &&
      (!jetton_address || jetton_address.length === 0)
    )
      return []

    const addresses = owner_address || jetton_address || []
    const paramName = owner_address ? "owner_address" : "jetton_address"

    const results = await Promise.all(
      addresses.map(async addr => {
        const url = this.buildUrl(this.v3BaseUrl, "/jetton/wallets")
        url.searchParams.append(paramName, addr)
        try {
          const response = await this.request<{jetton_wallets: JettonWallet[]}>(
            url,
            "Failed to fetch jetton wallets",
          )
          return response.jetton_wallets
        } catch (error) {
          console.error(`Failed to fetch jetton wallets for ${addr}`, error)
          return []
        }
      }),
    )

    return results.flat()
  }

  async getTraces(hash: string): Promise<V3TracesResponse> {
    const url = this.buildUrl(this.v3BaseUrl, "/traces")
    url.searchParams.append("hash", hash)
    return this.request(url, "Failed to fetch traces")
  }

  async getAddressName(address: string): Promise<string | undefined> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/address-name")
    url.searchParams.append("address", address)
    return this.request(url, "Failed to fetch address name")
  }

  async setAddressName(address: string, name: string): Promise<void> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/address-name")
    await this.request<null>(url, "Failed to set address name", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({address, name}),
    })
  }

  private buildUrl(base: string, path: string): URL {
    const fullBase = base.startsWith("http") ? base : `${globalThis.location.origin}${base}`
    return new URL(`${fullBase}${path}`)
  }

  private async request<T>(url: URL, errorMessage: string, options?: RequestInit): Promise<T> {
    const response = await fetch(url.toString(), options)
    const data = (await response.json()) as ApiResponse<T>
    if (!data.ok) throw new Error(data.error || errorMessage)
    return data.result
  }
}

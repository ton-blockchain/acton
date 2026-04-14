import type {
  AccountStatesResponse,
  ApiResponse,
  FullAccountState,
  JettonMaster,
  JettonWallet,
  NftItem,
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

  async getAccountStates(addresses: string[], includeBoc = true): Promise<AccountStatesResponse> {
    const url = this.buildUrl(this.v3BaseUrl, "/accountStates")
    for (const address of addresses) {
      url.searchParams.append("address", address)
    }
    url.searchParams.append("include_boc", includeBoc ? "true" : "false")
    return this.request(url, "Failed to fetch account states")
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

  async getNftItems(options?: {
    readonly address?: string[]
    readonly owner_address?: string[]
    readonly collection_address?: string[]
    readonly sortByLastTransactionLt?: boolean
    readonly limit?: number
    readonly offset?: number
  }): Promise<NftItem[]> {
    const addresses = options?.address
    const ownerAddresses = options?.owner_address
    const collectionAddresses = options?.collection_address
    const sortByLastTransactionLt = options?.sortByLastTransactionLt || false
    const limit = options?.limit ?? 100
    const offset = options?.offset ?? 0

    const buildAndFetch = async (paramName?: string, value?: string): Promise<NftItem[]> => {
      const url = this.buildUrl(this.v3BaseUrl, "/nft/items")
      if (paramName && value) {
        url.searchParams.append(paramName, value)
      }
      url.searchParams.append("limit", limit.toString())
      url.searchParams.append("offset", offset.toString())
      if (sortByLastTransactionLt) {
        url.searchParams.append("sort_by_last_transaction_lt", "true")
      }

      const response = await this.request<{nft_items: NftItem[]}>(url, "Failed to fetch NFTs")
      return response.nft_items
    }

    if (addresses && addresses.length > 0) {
      const results = await Promise.all(
        addresses.map(async addr => {
          try {
            return await buildAndFetch("address", addr)
          } catch (error) {
            console.error(`Failed to fetch NFT for ${addr}`, error)
            return []
          }
        }),
      )
      return this.dedupNftItems(results.flat())
    }

    if (ownerAddresses && ownerAddresses.length > 0) {
      const results = await Promise.all(
        ownerAddresses.map(async owner => {
          try {
            return await buildAndFetch("owner_address", owner)
          } catch (error) {
            console.error(`Failed to fetch NFTs for owner ${owner}`, error)
            return []
          }
        }),
      )
      return this.dedupNftItems(results.flat())
    }

    if (collectionAddresses && collectionAddresses.length > 0) {
      const results = await Promise.all(
        collectionAddresses.map(async collection => {
          try {
            return await buildAndFetch("collection_address", collection)
          } catch (error) {
            console.error(`Failed to fetch NFTs for collection ${collection}`, error)
            return []
          }
        }),
      )
      return this.dedupNftItems(results.flat())
    }

    return buildAndFetch()
  }

  async getAddressName(address: string): Promise<string | undefined> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/admin/address-name")
    url.searchParams.append("address", address)
    return this.request(url, "Failed to fetch address name")
  }

  async setAddressName(address: string, name: string): Promise<void> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/admin/address-name")
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
    const raw = (await response.json()) as unknown

    if (this.isApiResponse<T>(raw)) {
      if (!raw.ok) {
        throw new Error(raw.error || errorMessage)
      }
      return raw.result
    }

    if (!response.ok) {
      throw new Error(this.extractError(raw) || errorMessage)
    }

    if (this.isRequestError(raw)) {
      throw new Error(raw.error || errorMessage)
    }

    return raw as T
  }

  private dedupNftItems(items: NftItem[]): NftItem[] {
    const seen = new Map<string, NftItem>()
    for (const item of items) {
      if (!seen.has(item.address)) {
        seen.set(item.address, item)
      }
    }
    return [...seen.values()]
  }

  private isApiResponse<T>(value: unknown): value is ApiResponse<T> {
    return (
      typeof value === "object" &&
      value !== null &&
      "ok" in value &&
      typeof (value as {ok: unknown}).ok === "boolean"
    )
  }

  private isRequestError(value: unknown): value is {error?: string; code?: number} {
    return typeof value === "object" && value !== null && "error" in value && "code" in value
  }

  private extractError(value: unknown): string | undefined {
    if (typeof value !== "object" || value === null || !("error" in value)) {
      return undefined
    }
    const error = (value as {error?: unknown}).error
    return typeof error === "string" ? error : undefined
  }
}

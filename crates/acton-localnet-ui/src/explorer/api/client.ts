import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {Cell} from "@ton/core"

import type {
  AccountStatesResponse,
  ApiResponse,
  FullAccountState,
  JettonMaster,
  JettonWallet,
  JettonWalletData,
  LocalnetNodeInfo,
  NftItem,
  StartupWallet,
  StreamingTransactionsEvent,
  Transaction,
  V3RunGetMethodResponse,
  V3RunGetMethodStackEntry,
  V3TracesResponse,
  V3TransactionsResponse,
} from "./types"

interface TonClientOptions {
  readonly v2BaseUrl: string
  readonly v3BaseUrl: string
  readonly addressNameBaseUrl: string
  readonly toncenterApiKey?: string
}

interface FaucetResponse {
  readonly ok?: boolean
  readonly success?: boolean
  readonly error?: string
}

interface TransactionStreamHandlers {
  readonly onTransactions: (event: StreamingTransactionsEvent) => void
  readonly onError?: (error: Error) => void
}

export class TonClient {
  private readonly v2BaseUrl: string
  private readonly v3BaseUrl: string
  private readonly addressNameBaseUrl: string
  private readonly toncenterApiKey: string | undefined
  private readonly pendingGetRequests = new Map<string, Promise<unknown>>()

  constructor({v2BaseUrl, v3BaseUrl, addressNameBaseUrl, toncenterApiKey}: TonClientOptions) {
    this.v2BaseUrl = v2BaseUrl
    this.v3BaseUrl = v3BaseUrl
    this.addressNameBaseUrl = addressNameBaseUrl
    this.toncenterApiKey = toncenterApiKey?.trim() || undefined
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

  subscribeAccountTransactions(address: string, handlers: TransactionStreamHandlers): () => void {
    const controller = new AbortController()
    void this.readAccountTransactionStream(address, handlers, controller.signal)
    return () => controller.abort()
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

    return this.fetchJettonWallets(paramName, addresses)
  }

  async getJettonWalletsByAddress(address: string[]): Promise<JettonWallet[]> {
    if (address.length === 0) return []
    return this.fetchJettonWallets("address", address)
  }

  async runGetMethod(
    address: string,
    method: string | number,
    stack: readonly V3RunGetMethodStackEntry[] = [],
    seqno?: number,
  ): Promise<V3RunGetMethodResponse> {
    const url = this.buildUrl(this.v3BaseUrl, "/runGetMethod")
    const body: {
      readonly address: string
      readonly method: string | number
      readonly stack: readonly V3RunGetMethodStackEntry[]
      readonly seqno?: number
    } = seqno === undefined ? {address, method, stack} : {address, method, stack, seqno}

    return this.request(url, "Failed to run get method", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify(body),
    })
  }

  async getJettonWalletData(
    address: string,
    seqno?: number,
  ): Promise<JettonWalletData | undefined> {
    const response = await this.runGetMethod(address, "get_wallet_data", [], seqno)
    if (response.exit_code !== 0) {
      return undefined
    }

    const balance = this.stackNumber(response.stack[0])
    const owner = this.stackAddress(response.stack[1])
    const jetton = this.stackAddress(response.stack[2])
    if (balance === undefined || owner === undefined || jetton === undefined) {
      return undefined
    }

    return {balance, owner, jetton}
  }

  private async fetchJettonWallets(
    paramName: "address" | "owner_address" | "jetton_address",
    addresses: string[],
  ): Promise<JettonWallet[]> {
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

  async getRecentTransactions(limit = 10): Promise<V3TransactionsResponse> {
    const url = this.buildUrl(this.v3BaseUrl, "/transactions")
    url.searchParams.append("limit", limit.toString())
    return this.request(url, "Failed to fetch recent transactions")
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

  async getAddressNames(addresses: readonly string[]): Promise<Record<string, string | undefined>> {
    const uniqueAddresses = [...new Set(addresses.filter(Boolean))]
    if (uniqueAddresses.length === 0) {
      return {}
    }

    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_getAddressName")
    for (const address of uniqueAddresses) {
      url.searchParams.append("address", address)
    }
    const response = await this.request<Record<string, string | null>>(
      url,
      "Failed to fetch address names",
    )

    return Object.fromEntries(
      Object.entries(response).map(([address, name]) => [address, name ?? undefined]),
    )
  }

  async getCompilerAbis(
    codeHashes: readonly string[],
  ): Promise<Record<string, ContractABI | null>> {
    const uniqueCodeHashes = [...new Set(codeHashes.filter(Boolean))]
    if (uniqueCodeHashes.length === 0) {
      return {}
    }

    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_getCompilerAbi")
    for (const codeHash of uniqueCodeHashes) {
      url.searchParams.append("code_hash", codeHash)
    }
    return this.request<Record<string, ContractABI | null>>(url, "Failed to fetch compiler ABI")
  }

  async getNodeInfo(): Promise<LocalnetNodeInfo> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_nodeInfo")
    return this.request(url, "Failed to fetch node info")
  }

  async getStartupWallets(): Promise<StartupWallet[]> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_getStartupWallets")
    return this.request(url, "Failed to fetch startup wallets")
  }

  async setAddressName(address: string, name: string): Promise<void> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_setAddressName")
    await this.request<null>(url, "Failed to set address name", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({address, name}),
    })
  }

  async fundAccount(address: string, amount: number): Promise<void> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_fundAccount")
    const response = await this.request<FaucetResponse>(url, "Failed to fund account", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({address, amount}),
    })

    if (response.ok === false || response.success === false) {
      throw new Error(response.error || "Failed to fund account")
    }
  }

  async setShardAccount(address: string, shardAccount: string): Promise<void> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_setShardAccount")
    await this.request<null>(url, "Failed to set shard account", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({address, shard_account: shardAccount}),
    })
  }

  async sendInternalMessage(boc: string): Promise<void> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_sendInternalMessage")
    await this.request<unknown>(url, "Failed to send internal message", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({boc}),
    })
  }

  getEndpoints(): {readonly apiV2: string; readonly apiV3: string; readonly admin: string} {
    return {
      apiV2: this.buildUrl(this.v2BaseUrl, "").toString().replace(/\/$/, ""),
      apiV3: this.buildUrl(this.v3BaseUrl, "").toString().replace(/\/$/, ""),
      admin: this.buildUrl(this.addressNameBaseUrl, "").toString().replace(/\/$/, ""),
    }
  }

  private buildUrl(base: string, path: string): URL {
    const fullBase = base.startsWith("http") ? base : `${globalThis.location.origin}${base}`
    return new URL(`${fullBase}${path}`)
  }

  private buildStreamingSseUrl(): URL {
    const url = this.buildUrl(this.v2BaseUrl, "")
    const apiRoot = url.pathname.replace(/\/$/, "").replace(/\/v2$/, "")
    url.pathname = `${apiRoot}/streaming/v2/sse`
    url.search = ""
    url.hash = ""
    return url
  }

  private async readAccountTransactionStream(
    address: string,
    handlers: TransactionStreamHandlers,
    signal: AbortSignal,
  ): Promise<void> {
    try {
      const url = this.buildStreamingSseUrl()
      const response = await fetch(
        url.toString(),
        this.withToncenterApiKey(url, {
          method: "POST",
          headers: {
            Accept: "text/event-stream",
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            addresses: [address],
            types: ["transactions"],
            min_finality: "confirmed",
          }),
          signal,
        }),
      )

      if (!response.ok) {
        const body = await response.text().catch(() => "")
        throw new Error(body || `Streaming subscription failed with status ${response.status}`)
      }
      if (!response.body) {
        throw new Error("Streaming subscription returned an empty body")
      }

      await this.readSseEvents(response.body, value => {
        if (isStreamingTransactionsEvent(value)) {
          handlers.onTransactions(value)
        }
      })
    } catch (error) {
      if (signal.aborted) return
      handlers.onError?.(error instanceof Error ? error : new Error(String(error)))
    }
  }

  private async readSseEvents(
    body: ReadableStream<Uint8Array>,
    onEvent: (value: unknown) => void,
  ): Promise<void> {
    const reader = body.getReader()
    const decoder = new TextDecoder()
    let buffer = ""
    let dataLines: string[] = []

    const dispatch = () => {
      if (dataLines.length === 0) {
        return
      }

      const data = dataLines.join("\n")
      dataLines = []
      try {
        onEvent(JSON.parse(data) as unknown)
      } catch (error) {
        console.debug("Failed to parse streaming event", error)
      }
    }

    const processLine = (line: string) => {
      if (line.length === 0) {
        dispatch()
        return
      }
      if (line.startsWith("data:")) {
        dataLines.push(line.slice(5).trimStart())
      }
    }

    while (true) {
      const {value, done} = await reader.read()
      if (done) {
        buffer += decoder.decode()
        break
      }

      buffer += decoder.decode(value, {stream: true})
      const lines = buffer.split(/\r?\n/)
      buffer = lines.pop() ?? ""
      for (const line of lines) {
        processLine(line)
      }
    }

    if (buffer.length > 0) {
      processLine(buffer)
    }
    dispatch()
  }

  private async request<T>(url: URL, errorMessage: string, options?: RequestInit): Promise<T> {
    const dedupeKey = this.pendingRequestKey(url, options)
    if (dedupeKey) {
      const pending = this.pendingGetRequests.get(dedupeKey)
      if (pending) {
        return pending as Promise<T>
      }

      const request = this.fetchRequest<T>(url, errorMessage, options).finally(() => {
        this.clearPendingGetRequest(dedupeKey, request)
      })
      this.pendingGetRequests.set(dedupeKey, request)
      return request
    }

    return this.fetchRequest<T>(url, errorMessage, options)
  }

  private async fetchRequest<T>(url: URL, errorMessage: string, options?: RequestInit): Promise<T> {
    const response = await fetch(url.toString(), this.withToncenterApiKey(url, options))
    const raw = await this.parseResponseJson(response, errorMessage)

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

  private pendingRequestKey(url: URL, options?: RequestInit): string | undefined {
    const method = options?.method?.toUpperCase() ?? "GET"
    return method === "GET" ? url.toString() : undefined
  }

  private clearPendingGetRequest(key: string, request: Promise<unknown>): void {
    if (this.pendingGetRequests.get(key) === request) {
      this.pendingGetRequests.delete(key)
    }
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

  private async parseResponseJson(response: Response, errorMessage: string): Promise<unknown> {
    const text = await response.text()
    if (text.length === 0) {
      return undefined
    }

    try {
      return JSON.parse(text) as unknown
    } catch {
      throw new Error(
        `${errorMessage}: received non-JSON response from ${new URL(response.url).pathname}`,
      )
    }
  }

  private withToncenterApiKey(url: URL, options?: RequestInit): RequestInit | undefined {
    if (!this.toncenterApiKey || !this.isToncenterApiUrl(url)) {
      return options
    }

    const headers = new Headers(options?.headers)
    headers.set("X-API-Key", this.toncenterApiKey)
    return {...options, headers}
  }

  private isToncenterApiUrl(url: URL): boolean {
    return (
      this.isUrlWithinBase(url, this.buildUrl(this.v2BaseUrl, "")) ||
      this.isUrlWithinBase(url, this.buildUrl(this.v3BaseUrl, ""))
    )
  }

  private isUrlWithinBase(url: URL, baseUrl: URL): boolean {
    const basePath = baseUrl.pathname.replace(/\/$/, "")
    return (
      url.origin === baseUrl.origin &&
      (url.pathname === basePath || url.pathname.startsWith(`${basePath}/`))
    )
  }

  private stackNumber(entry: V3RunGetMethodStackEntry | undefined): string | undefined {
    if (entry?.type !== "num") return undefined
    if (typeof entry.value === "string") {
      try {
        return BigInt(entry.value).toString()
      } catch {
        return undefined
      }
    }
    if (typeof entry.value === "number") {
      return Math.trunc(entry.value).toString()
    }
    return undefined
  }

  private stackAddress(entry: V3RunGetMethodStackEntry | undefined): string | undefined {
    if (entry?.type !== "slice" || typeof entry.value !== "string") {
      return undefined
    }

    try {
      return Cell.fromBase64(entry.value).beginParse().loadAddress()?.toString()
    } catch {
      return undefined
    }
  }
}

function isStreamingTransactionsEvent(value: unknown): value is StreamingTransactionsEvent {
  if (typeof value !== "object" || value === null) {
    return false
  }

  const event = value as Partial<StreamingTransactionsEvent>
  return (
    event.type === "transactions" &&
    (event.finality === "pending" ||
      event.finality === "confirmed" ||
      event.finality === "finalized") &&
    Array.isArray(event.transactions)
  )
}

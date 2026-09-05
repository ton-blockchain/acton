import {Cell} from "@ton/core"

import {hashToHex} from "../components/utils"
import {addressKey, type ExtendedContractABI} from "./compilerAbi"
import {parseNetworkConfig, type NetworkConfig} from "./config"
import {
  parseSuspendedAccountsConfig,
  readSuspendedAccountsConfigCache,
  type SuspendedAccountsConfig,
  writeSuspendedAccountsConfigCache,
} from "./suspendedAccounts"
import type {
  AddressInformation,
  AccountStateTokenInfo,
  AccountStatesResponse,
  ApiResponse,
  BuildSourceTraceRequest,
  JettonMaster,
  JettonMasterMetadata,
  JettonTransfer,
  JettonWallet,
  JettonWalletData,
  LocalnetCheckpoint,
  LocalnetContract,
  LocalnetMineResult,
  LocalnetMiningMode,
  LocalnetNetworkConditions,
  LocalnetNodeInfo,
  LocalnetSetConfigResult,
  LocalnetTimeInfo,
  NftItem,
  Shards,
  StreamingActionsEvent,
  StreamingTransactionsEvent,
  SourceTraceResponse,
  V3Action,
  V3ActionsResponse,
  V3BlocksResponse,
  V3MultisigOrdersResponse,
  V3MultisigWalletsResponse,
  V3Metadata,
  V3RunGetMethodResponse,
  V3RunGetMethodStackEntry,
  V3TransactionDetailsResponse,
  V3TracesResponse,
  V3TransactionsResponse,
  V2BlockTransactionsResponse,
  VerificationSourceResponse,
} from "./types"
import {v2ShardToV3Shard, v3ShardToV2Shard} from "./shardId"

interface TonClientOptions {
  readonly v2BaseUrl: string
  readonly v3BaseUrl: string
  readonly toncenterProxyV2BaseUrl?: string
  readonly toncenterProxyV3BaseUrl?: string
  readonly addressNameBaseUrl: string
  readonly localnetControlEnabled?: boolean
  readonly toncenterApiCompatible?: boolean
  readonly localnetApiToken?: string
  readonly onUnauthorized?: () => void
  readonly toncenterApiKey?: string
}

type NodeAddressInformation = Omit<AddressInformation, "status"> & {
  readonly state: AddressInformation["status"]
}

const REQUEST_SOURCE_HEADER = "X-Acton-Request-Source"
const STUDIO_UI_REQUEST_SOURCE = "studio-ui"

export type AccountHistorySortOrder = "asc" | "desc"
export type RawBlockNetwork = "mainnet" | "testnet"

export interface GetAccountActionsOptions {
  readonly traceId?: string
  readonly startLt?: string
  readonly endLt?: string
}

export interface GetNftItemsOptions {
  readonly address?: string[]
  readonly owner_address?: string[]
  readonly collection_address?: string[]
  readonly sortByLastTransactionLt?: boolean
  readonly limit?: number
  readonly offset?: number
}

export interface NftItemsPage {
  readonly items: NftItem[]
  readonly rawItemCount: number
}

export type CompilerAbiLoader = (
  codeHashes: readonly string[],
) => Promise<Record<string, ExtendedContractABI | null>>

interface FaucetResponse {
  readonly ok?: boolean
  readonly success?: boolean
  readonly error?: string
  readonly hash?: string
}

interface SendInternalMessageResponse {
  readonly hash: string
}

interface SendExternalMessageResponse {
  readonly hash: string
}

interface DnsRecordsResponse {
  readonly records: readonly {
    readonly domain: string
    readonly dns_wallet?: string | null
  }[]
}

interface DnsResolvedResponse {
  readonly entries: readonly {
    readonly entry: {
      readonly "@type": string
      readonly smc_address?: {
        readonly account_address?: string
      }
    }
  }[]
}

interface GetBlocksOptions {
  readonly workchain?: number
  readonly shard?: string
  readonly seqno?: number
  readonly rootHash?: string
  readonly fileHash?: string
  readonly mcSeqno?: number
  readonly startUtime?: number
  readonly endUtime?: number
  readonly startLt?: string | number
  readonly endLt?: string | number
  readonly limit?: number
  readonly offset?: number
  readonly sort?: "asc" | "desc"
}

interface GetBlockTransactionsOptions {
  readonly workchain: number
  readonly shard: string
  readonly seqno: number
  readonly limit?: number
  readonly offset?: number
}

interface GetBlockTransactionsV2Options {
  readonly workchain: number
  readonly shard: string
  readonly seqno: number
  readonly rootHash?: string
  readonly fileHash?: string
  readonly count?: number
  readonly afterLt?: string
  readonly afterHash?: string
}

interface RawBlockResponse {
  readonly data: string
}

export interface RawBlockReference {
  readonly workchain: number
  readonly shard: string
  readonly seqno: number
  readonly root_hash: string
  readonly file_hash: string
}

export function buildToncoinBlockDownloadUrl(
  toncoinOrigin: string,
  block: RawBlockReference,
): URL | undefined {
  const rootHash = hashToHex(block.root_hash)
  const fileHash = hashToHex(block.file_hash)
  if (!rootHash || !fileHash) return undefined

  const url = new URL("/download", toncoinOrigin)
  url.searchParams.append("workchain", block.workchain.toString())
  url.searchParams.append("shard", block.shard)
  url.searchParams.append("seqno", block.seqno.toString())
  url.searchParams.append("roothash", rootHash.toUpperCase())
  url.searchParams.append("filehash", fileHash.toUpperCase())
  return url
}

interface GetTracesOptions {
  readonly includeActions?: boolean
}

interface GetJettonWalletsOptions {
  readonly limit?: number
  readonly offset?: number
  readonly sort?: "asc" | "desc"
}

type JettonWalletMetadata = Record<
  string,
  {
    readonly token_info?: readonly AccountStateTokenInfo[]
  }
>

interface JettonWalletsResponse {
  readonly jetton_wallets: JettonWallet[]
  readonly metadata?: JettonWalletMetadata
}

interface JettonMastersResponse {
  readonly jetton_masters: JettonMaster[]
  readonly metadata?: JettonWalletMetadata
}

interface JettonTransfersResponse {
  readonly jetton_transfers: JettonTransfer[]
}

interface NftItemsResponse {
  readonly nft_items: NftItem[]
  readonly metadata?: JettonWalletMetadata
}

interface V2ConfigInfo {
  readonly config: {
    readonly bytes: string
  }
}

const IMAGE_CONTENT_KEYS = ["_image_small", "_image_medium", "_image_big", "image"] as const
const JETTON_CONTENT_KEYS = [
  "uri",
  "name",
  "description",
  ...IMAGE_CONTENT_KEYS,
  "symbol",
  "decimals",
] as const
const NFT_CONTENT_KEYS = [
  "uri",
  "name",
  "description",
  ...IMAGE_CONTENT_KEYS,
  "preview",
  "image_url",
  "symbol",
  "collection",
  "collection_name",
] as const

const TON_DNS_ROOT_ADDRESS = "-1:e56754f83426f69b09267bd876ac97c44821345b7e266bd956a7bfbfb98df35c"
const DNS_RESOLVE_TTL = 10

function jettonMasterMetadataFromWalletResponse(
  jettonAddress: string,
  metadata: JettonWalletMetadata | undefined,
): JettonMasterMetadata | undefined {
  const tokenInfo = metadata?.[jettonAddress]?.token_info?.find(
    info => info.type === "jetton_masters",
  )
  if (!tokenInfo) {
    return undefined
  }

  const extra = isRecord(tokenInfo.extra) ? tokenInfo.extra : {}
  const jettonContent: Record<string, unknown> = {...extra}
  for (const key of JETTON_CONTENT_KEYS) {
    const value = stringValue(tokenInfo[key]) ?? stringValue(extra[key])
    if (value) {
      jettonContent[key] = value
    }
  }

  const totalSupply = stringValue(tokenInfo.total_supply) ?? stringValue(extra.total_supply)
  const mintable = booleanValue(tokenInfo.mintable) ?? booleanValue(extra.mintable)
  if (
    Object.keys(jettonContent).length === 0 &&
    totalSupply === undefined &&
    mintable === undefined
  ) {
    return undefined
  }

  return {
    address: jettonAddress,
    jetton_content: jettonContent,
    ...(totalSupply ? {total_supply: totalSupply} : undefined),
    ...(mintable === undefined ? undefined : {mintable}),
  }
}

const ACTION_JETTON_ASSET_KEYS = new Set([
  "asset",
  "asset_in",
  "asset_out",
  "asset_1",
  "asset_2",
  "source_asset",
  "target_asset_1",
  "target_asset_2",
])

function collectActionAssetAddresses(actions: readonly V3Action[]): string[] {
  const addresses = new Set<string>()

  const visit = (value: unknown, key?: string): void => {
    if (key && ACTION_JETTON_ASSET_KEYS.has(key) && stringValue(value)) {
      addresses.add(value as string)
    }

    if (Array.isArray(value)) {
      for (const item of value) {
        visit(item)
      }
      return
    }

    if (isRecord(value)) {
      for (const [childKey, childValue] of Object.entries(value)) {
        visit(childValue, childKey)
      }
    }
  }

  for (const action of actions) {
    visit(action.details)
  }

  return [...addresses]
}

function metadataTokenInfoForAddress(
  metadata: V3Metadata,
  address: string,
): AccountStateTokenInfo | undefined {
  const entry = metadata[address] ?? metadata[addressKey(address)]
  return entry?.token_info?.find(info => info.type === "jetton_masters")
}

function metadataTokenHasSymbol(tokenInfo: AccountStateTokenInfo | undefined): boolean {
  const extra = isRecord(tokenInfo?.extra) ? tokenInfo.extra : {}
  return stringValue(tokenInfo?.symbol) !== undefined || stringValue(extra.symbol) !== undefined
}

function mergeJettonMastersIntoMetadata(
  metadata: V3Metadata,
  masters: readonly JettonMaster[],
): V3Metadata {
  let mergedMetadata = metadata

  for (const master of masters) {
    const normalizedKey = addressKey(master.address)
    const key = mergedMetadata[master.address] ? master.address : normalizedKey
    const existing = mergedMetadata[key]
    const existingTokenInfo = existing?.token_info ?? []
    const masterTokenInfo: AccountStateTokenInfo = {
      ...(existingTokenInfo.find(info => info.type === "jetton_masters") ?? {}),
      type: "jetton_masters",
      ...master.jetton_content,
      total_supply: master.total_supply,
      mintable: master.mintable,
    }

    mergedMetadata = {
      ...mergedMetadata,
      [key]: {
        ...(existing ?? {}),
        token_info: existingTokenInfo.some(info => info.type === "jetton_masters")
          ? existingTokenInfo.map(info => (info.type === "jetton_masters" ? masterTokenInfo : info))
          : [...existingTokenInfo, masterTokenInfo],
      },
    }
  }

  return mergedMetadata
}

function attachJettonMasterMetadata(
  master: JettonMaster,
  metadata: JettonWalletMetadata | undefined,
): JettonMaster {
  const normalizedMetadata = jettonMasterMetadataFromWalletResponse(master.address, metadata)
  if (!normalizedMetadata) {
    return master
  }

  return {
    ...master,
    jetton_content: {
      ...master.jetton_content,
      ...normalizedMetadata.jetton_content,
    },
  }
}

function attachNftItemMetadata(item: NftItem, metadata: JettonWalletMetadata | undefined): NftItem {
  const tokenInfo = metadata?.[item.address]?.token_info?.find(info => info.type === "nft_items")
  const tokenExtra = isRecord(tokenInfo?.extra) ? tokenInfo.extra : {}
  const content: Record<string, unknown> = {...tokenExtra}
  const isNsfw = booleanValue(tokenInfo?.is_nsfw)
  const isScam = booleanValue(tokenInfo?.is_scam)

  if (tokenInfo) {
    for (const key of NFT_CONTENT_KEYS) {
      const value = stringValue(tokenInfo[key]) ?? stringValue(tokenExtra[key])
      if (value) {
        content[key] = value
      }
    }
  }

  const collectionAddress = item.collection?.address ?? item.collection_address
  const collectionInfo = collectionAddress
    ? metadata?.[collectionAddress]?.token_info?.find(info => info.type === "nft_collections")
    : undefined
  const collectionExtra = isRecord(collectionInfo?.extra) ? collectionInfo.extra : {}
  const collectionName =
    stringValue(collectionInfo?.name) ??
    stringValue(collectionExtra.name) ??
    stringValue(item.collection?.collection_content?.name)
  if (collectionName && !stringValue(content.collection_name)) {
    content.collection_name = collectionName
  }
  const domainName = stringValue(content.domain)
  if (domainName && !stringValue(content.name)) {
    content.name = domainName
  }

  if (Object.keys(content).length === 0 && isNsfw === undefined && isScam === undefined) {
    return item
  }

  return {
    ...item,
    ...(isNsfw === undefined ? {} : {is_nsfw: isNsfw}),
    ...(isScam === undefined ? {} : {is_scam: isScam}),
    content: {
      ...item.content,
      ...content,
    },
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined
}

function booleanValue(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined
}

interface AccountHistoryStreamHandlers {
  readonly onTransactions: (event: StreamingTransactionsEvent) => void
  readonly onActions?: (event: StreamingActionsEvent) => void
  readonly onError?: (error: Error) => void
}

function isToncenterApiBaseUrl(baseUrl: string): boolean {
  try {
    const fullBase = baseUrl.startsWith("http")
      ? baseUrl
      : `${globalThis.location.origin}${baseUrl}`
    const apiUrl = new URL(fullBase)
    return apiUrl.hostname === "toncenter.com" || apiUrl.hostname.endsWith(".toncenter.com")
  } catch {
    return false
  }
}

export class TonClient {
  private readonly v2BaseUrl: string
  private readonly v3BaseUrl: string
  private readonly toncenterProxyV2BaseUrl: string
  private readonly toncenterProxyV3BaseUrl: string
  private readonly addressNameBaseUrl: string
  private readonly localnetControlEnabled: boolean
  private readonly toncenterApiCompatible: boolean
  private readonly localnetApiToken: string | undefined
  private readonly onUnauthorized: (() => void) | undefined
  private readonly toncenterApiKey: string | undefined
  private readonly pendingGetRequests = new Map<string, Promise<unknown>>()

  constructor({
    v2BaseUrl,
    v3BaseUrl,
    toncenterProxyV2BaseUrl,
    toncenterProxyV3BaseUrl,
    addressNameBaseUrl,
    localnetControlEnabled = true,
    toncenterApiCompatible,
    localnetApiToken,
    onUnauthorized,
    toncenterApiKey,
  }: TonClientOptions) {
    this.v2BaseUrl = v2BaseUrl
    this.v3BaseUrl = v3BaseUrl
    this.toncenterProxyV2BaseUrl = toncenterProxyV2BaseUrl ?? v2BaseUrl
    this.toncenterProxyV3BaseUrl = toncenterProxyV3BaseUrl ?? v3BaseUrl
    this.addressNameBaseUrl = addressNameBaseUrl
    this.localnetControlEnabled = localnetControlEnabled
    this.toncenterApiCompatible = toncenterApiCompatible ?? isToncenterApiBaseUrl(v3BaseUrl)
    this.localnetApiToken = localnetApiToken?.trim() || undefined
    this.onUnauthorized = onUnauthorized
    this.toncenterApiKey = toncenterApiKey?.trim() || undefined
  }

  async getAddressInformation(address: string): Promise<AddressInformation> {
    const url = this.buildUrl(this.v3BaseUrl, "/addressInformation")
    url.searchParams.append("address", address)
    url.searchParams.append("include_boc", "true")
    const indexed = await this.request<AddressInformation>(
      url,
      "Failed to fetch address information",
    )
    if (
      indexed.balance !== "0" ||
      indexed.code !== null ||
      indexed.data !== null ||
      !["uninitialized", "uninit", "nonexist"].includes(indexed.status)
    ) {
      return indexed
    }

    const nodeUrl = this.buildUrl(this.v2BaseUrl, "/getAddressInformation")
    nodeUrl.searchParams.append("address", address)
    try {
      const node = await this.request<NodeAddressInformation>(
        nodeUrl,
        "Failed to fetch address information from the node",
      )
      const {state, ...information} = node
      return {...information, status: state}
    } catch {
      return indexed
    }
  }

  async getSuspendedAccountsConfig(): Promise<SuspendedAccountsConfig> {
    const cached = readSuspendedAccountsConfigCache(this.v2BaseUrl)
    if (cached) return cached

    const url = this.buildUrl(this.v2BaseUrl, "/getConfigParam")
    url.searchParams.append("config_id", "44")
    const response = await this.request<V2ConfigInfo>(
      url,
      "Failed to fetch suspended accounts config",
    )
    const config = parseSuspendedAccountsConfig(response.config.bytes)
    writeSuspendedAccountsConfigCache(this.v2BaseUrl, config)
    return config
  }

  async getNetworkConfig(seqno?: number): Promise<NetworkConfig> {
    const url = this.buildUrl(this.v2BaseUrl, "/getConfigAll")
    if (seqno !== undefined) {
      url.searchParams.append("seqno", String(seqno))
    }

    const response = await this.request<V2ConfigInfo>(url, "Failed to fetch network configuration")
    return parseNetworkConfig(response.config.bytes)
  }

  async resolveDnsWalletAddress(domain: string): Promise<string | undefined> {
    const url = this.buildUrl(this.v3BaseUrl, "/dns/records")
    url.searchParams.append("domain", domain)
    const response = await this.request<DnsRecordsResponse>(url, "Failed to resolve TON DNS name")
    const walletAddress = response.records.find(record => record.dns_wallet)?.dns_wallet
    if (walletAddress || response.records.length > 0) {
      return walletAddress ?? undefined
    }

    const resolved = await this.requestDnsFromChain(domain)
    return resolved.entries
      .filter(entry => entry.entry["@type"] === "dns.entryDataSmcAddress")
      .map(entry => stringValue(entry.entry.smc_address?.account_address))
      .find(address => address !== undefined)
  }

  async getWalletDnsNames(address: string): Promise<readonly string[]> {
    const url = this.buildUrl(this.v3BaseUrl, "/dns/records")
    url.searchParams.append("wallet", address)
    url.searchParams.append("limit", "1000")
    const response = await this.request<DnsRecordsResponse>(
      url,
      "Failed to load account TON DNS names",
    )
    return response.records.map(record => record.domain.trim())
  }

  async getAccountStates(addresses: string[], includeBoc = true): Promise<AccountStatesResponse> {
    const url = this.buildUrl(this.v3BaseUrl, "/accountStates")
    for (const address of addresses) {
      url.searchParams.append("address", address)
    }
    url.searchParams.append("include_boc", includeBoc ? "true" : "false")
    return this.request(url, "Failed to fetch account states")
  }

  async getShardAccountCell(address: string, seqno?: number): Promise<string> {
    const url = this.buildUrl(this.v2BaseUrl, "/getShardAccountCell")
    url.searchParams.append("address", address)
    if (seqno !== undefined) {
      url.searchParams.append("seqno", seqno.toString())
    }

    const response = await this.request<{readonly bytes?: string}>(
      url,
      "Failed to fetch shard account",
    )
    const boc = response.bytes
    if (!boc) {
      throw new Error("Shard account response does not contain a BoC")
    }
    return boc
  }

  async getAccountTransactions(
    address: string,
    limit = 20,
    offset = 0,
    sort: AccountHistorySortOrder = "desc",
  ): Promise<V3TransactionsResponse> {
    const url = this.buildUrl(this.v3BaseUrl, "/transactions")
    url.searchParams.append("account", address)
    url.searchParams.append("limit", limit.toString())
    if (offset > 0) {
      url.searchParams.append("offset", offset.toString())
    }
    url.searchParams.append("sort", sort)
    return this.request(url, "Failed to fetch account transactions")
  }

  subscribeAccountHistory(address: string, handlers: AccountHistoryStreamHandlers): () => void {
    const controller = new AbortController()
    void this.readAccountHistoryStream(address, handlers, controller.signal)
    return () => controller.abort()
  }

  async getJettonMasters(address?: string[], limit = 100, offset = 0): Promise<JettonMaster[]> {
    if (address && address.length > 0) {
      const url = this.buildUrl(this.v3BaseUrl, "/jetton/masters")
      for (const addr of address) {
        url.searchParams.append("address", addr)
      }
      url.searchParams.append("limit", Math.min(address.length, 1000).toString())
      const response = await this.request<JettonMastersResponse>(
        url,
        "Failed to fetch jetton masters",
      )
      return response.jetton_masters.map(master =>
        attachJettonMasterMetadata(master, response.metadata),
      )
    }

    const url = this.buildUrl(this.v3BaseUrl, "/jetton/masters")
    url.searchParams.append("limit", limit.toString())
    url.searchParams.append("offset", offset.toString())
    const response = await this.request<JettonMastersResponse>(
      url,
      "Failed to fetch jetton masters",
    )
    return response.jetton_masters.map(master =>
      attachJettonMasterMetadata(master, response.metadata),
    )
  }

  async getJettonTransfers(
    limit = 100,
    offset = 0,
    sort: AccountHistorySortOrder = "desc",
  ): Promise<JettonTransfer[]> {
    const url = this.buildUrl(this.v3BaseUrl, "/jetton/transfers")
    url.searchParams.append("limit", limit.toString())
    if (offset > 0) {
      url.searchParams.append("offset", offset.toString())
    }
    url.searchParams.append("sort", sort)
    const response = await this.request<JettonTransfersResponse>(
      url,
      "Failed to fetch jetton transfers",
    )
    return response.jetton_transfers
  }

  async getJettonWallets(
    owner_address?: string[],
    jetton_address?: string[],
    options: GetJettonWalletsOptions = {},
  ): Promise<JettonWallet[]> {
    if (
      (!owner_address || owner_address.length === 0) &&
      (!jetton_address || jetton_address.length === 0)
    )
      return []

    const addresses = owner_address || jetton_address || []
    const paramName = owner_address ? "owner_address" : "jetton_address"

    return this.fetchJettonWallets(paramName, addresses, options)
  }

  async getJettonWalletsByAddress(address: string[]): Promise<JettonWallet[]> {
    if (address.length === 0) return []
    return this.fetchJettonWallets("address", address)
  }

  async getMultisigWallets(
    addresses: readonly string[],
    includeOrders = false,
  ): Promise<V3MultisigWalletsResponse> {
    const url = this.buildUrl(this.v3BaseUrl, "/multisig/wallets")
    for (const address of addresses) {
      url.searchParams.append("address", address)
    }
    if (includeOrders) {
      url.searchParams.set("include_orders", "true")
    }
    return this.request(url, "Failed to fetch multisig wallets")
  }

  async getMultisigOrders(
    addresses: readonly string[],
    parseActions = false,
  ): Promise<V3MultisigOrdersResponse> {
    const url = this.buildUrl(this.v3BaseUrl, "/multisig/orders")
    for (const address of addresses) {
      url.searchParams.append("address", address)
    }
    if (parseActions) {
      url.searchParams.set("parse_actions", "true")
    }
    return this.request(url, "Failed to fetch multisig orders")
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
    options: GetJettonWalletsOptions = {},
  ): Promise<JettonWallet[]> {
    const results = await Promise.all(
      addresses.map(async addr => {
        const url = this.buildUrl(this.v3BaseUrl, "/jetton/wallets")
        url.searchParams.append(paramName, addr)
        if (options.limit !== undefined) {
          url.searchParams.append("limit", options.limit.toString())
        }
        if (options.offset !== undefined && options.offset > 0) {
          url.searchParams.append("offset", options.offset.toString())
        }
        if (options.sort) {
          url.searchParams.append("sort", options.sort)
        }
        const response = await this.request<JettonWalletsResponse>(
          url,
          "Failed to fetch jetton wallets",
        )
        return response.jetton_wallets.map(wallet =>
          this.attachJettonWalletMaster(wallet, response.metadata),
        )
      }),
    )

    return results.flat()
  }

  async getTraces(hash: string, options: GetTracesOptions = {}): Promise<V3TracesResponse> {
    const url = this.buildUrl(this.toncenterProxyV3BaseUrl, "/traces")
    url.searchParams.append("tx_hash", hash)
    if (options.includeActions) {
      url.searchParams.append("include_actions", "true")
    }
    return this.request(url, "Failed to fetch traces")
  }

  async getTransactionByHash(hash: string): Promise<V3TransactionDetailsResponse> {
    const url = this.buildUrl(this.v3BaseUrl, "/transactions")
    url.searchParams.append("hash", hash)
    url.searchParams.append("limit", "1")
    return this.request(url, "Failed to fetch transaction")
  }

  async getTransactionsByMessageHash(
    msgHash: string,
    direction?: "in" | "out",
  ): Promise<V3TransactionDetailsResponse> {
    const url = this.buildUrl(this.v3BaseUrl, "/transactionsByMessage")
    url.searchParams.append("msg_hash", msgHash)
    if (direction) {
      url.searchParams.append("direction", direction)
    }
    url.searchParams.append("limit", "1")
    return this.request(url, "Failed to fetch transaction by message")
  }

  async getAccountActions(
    address: string,
    limit = 20,
    offset = 0,
    sort: AccountHistorySortOrder = "desc",
    options: GetAccountActionsOptions = {},
  ): Promise<V3ActionsResponse> {
    const url = this.buildUrl(this.v3BaseUrl, "/actions")
    url.searchParams.append("account", address)
    if (options.traceId) {
      url.searchParams.append("trace_id", options.traceId)
    }
    if (options.startLt) {
      url.searchParams.append("start_lt", options.startLt)
    }
    if (options.endLt) {
      url.searchParams.append("end_lt", options.endLt)
    }
    url.searchParams.append("limit", limit.toString())
    if (offset > 0) {
      url.searchParams.append("offset", offset.toString())
    }
    url.searchParams.append("sort", sort)
    const response = await this.request<V3ActionsResponse>(url, "Failed to fetch account actions")
    const metadata = await this.enrichActionMetadata(response.actions, response.metadata)
    return metadata === response.metadata ? response : {...response, metadata}
  }

  async getTracesByMessageHash(msgHash: string): Promise<V3TracesResponse> {
    const url = this.buildUrl(this.v3BaseUrl, "/traces")
    url.searchParams.append("msg_hash", msgHash)
    return this.request(url, "Failed to fetch traces")
  }

  async getRecentTransactions(limit = 10): Promise<V3TransactionsResponse> {
    const url = this.buildUrl(this.v3BaseUrl, "/transactions")
    url.searchParams.append("limit", limit.toString())
    return this.request(url, "Failed to fetch recent transactions")
  }

  async getBlocks(options: GetBlocksOptions = {}): Promise<V3BlocksResponse> {
    const url = this.buildUrl(this.toncenterProxyV3BaseUrl, "/blocks")
    appendOptionalSearchParam(url, "workchain", options.workchain)
    appendOptionalSearchParam(url, "shard", options.shard)
    appendOptionalSearchParam(url, "seqno", options.seqno)
    appendOptionalSearchParam(url, "root_hash", options.rootHash)
    appendOptionalSearchParam(url, "file_hash", options.fileHash)
    appendOptionalSearchParam(url, "mc_seqno", options.mcSeqno)
    appendOptionalSearchParam(url, "start_utime", options.startUtime)
    appendOptionalSearchParam(url, "end_utime", options.endUtime)
    appendOptionalSearchParam(url, "start_lt", options.startLt)
    appendOptionalSearchParam(url, "end_lt", options.endLt)
    appendOptionalSearchParam(url, "limit", options.limit)
    appendOptionalSearchParam(url, "offset", options.offset)
    appendOptionalSearchParam(url, "sort", options.sort)
    return this.request(url, "Failed to fetch blocks")
  }

  async getRawBlockBoc(block: RawBlockReference): Promise<Cell> {
    const url = this.buildUrl(this.v2BaseUrl, "/getBlock")
    url.searchParams.append("workchain", block.workchain.toString())
    url.searchParams.append("shard", v3ShardToV2Shard(block.shard))
    url.searchParams.append("seqno", block.seqno.toString())
    url.searchParams.append("root_hash", block.root_hash)
    url.searchParams.append("file_hash", block.file_hash)
    url.searchParams.append("archival", "true")
    const response = await this.request<RawBlockResponse>(url, "Failed to fetch raw block")
    try {
      return Cell.fromBase64(response.data)
    } catch {
      throw new Error("Raw block response contains invalid BoC data")
    }
  }

  async getMasterchainBlockShards(seqno: number): Promise<V3BlocksResponse> {
    const url = this.buildUrl(this.toncenterProxyV2BaseUrl, "/getShards")
    url.searchParams.append("seqno", seqno.toString())
    const {shards} = await this.request<Shards>(url, "Failed to fetch masterchain block shards")
    const responses = await Promise.all(
      shards.map(shard =>
        this.getBlocks({
          workchain: shard.workchain,
          shard: v2ShardToV3Shard(shard.shard),
          seqno: shard.seqno,
          rootHash: shard.root_hash,
          fileHash: shard.file_hash,
          limit: 1,
        }),
      ),
    )
    return {blocks: responses.flatMap(response => response.blocks)}
  }

  async getBlockTransactions(
    options: GetBlockTransactionsOptions,
  ): Promise<V3TransactionsResponse> {
    const url = this.buildUrl(this.toncenterProxyV3BaseUrl, "/transactions")
    url.searchParams.append("workchain", options.workchain.toString())
    url.searchParams.append("shard", options.shard)
    url.searchParams.append("seqno", options.seqno.toString())
    url.searchParams.append("limit", (options.limit ?? 100).toString())
    if (options.offset !== undefined && options.offset > 0) {
      url.searchParams.append("offset", options.offset.toString())
    }
    return this.request(url, "Failed to fetch block transactions")
  }

  async getBlockTransactionsV2(
    options: GetBlockTransactionsV2Options,
  ): Promise<V2BlockTransactionsResponse> {
    const url = this.buildUrl(this.toncenterProxyV2BaseUrl, "/getBlockTransactions")
    url.searchParams.append("workchain", options.workchain.toString())
    url.searchParams.append("shard", v3ShardToV2Shard(options.shard))
    url.searchParams.append("seqno", options.seqno.toString())
    appendOptionalSearchParam(url, "root_hash", options.rootHash)
    appendOptionalSearchParam(url, "file_hash", options.fileHash)
    url.searchParams.append("count", (options.count ?? 100).toString())
    if (options.afterLt !== undefined && options.afterHash !== undefined) {
      url.searchParams.append("after_lt", options.afterLt)
      url.searchParams.append("after_hash", options.afterHash)
    }
    return this.request(url, "Failed to fetch fallback block transactions")
  }

  async getNftItems(options?: GetNftItemsOptions): Promise<NftItem[]> {
    const page = await this.getNftItemsPage(options)
    return page.items
  }

  async getNftItemsPage(options?: GetNftItemsOptions): Promise<NftItemsPage> {
    const addresses = options?.address
    const ownerAddresses = options?.owner_address
    const collectionAddresses = options?.collection_address
    const sortByLastTransactionLt = options?.sortByLastTransactionLt || false
    const limit = options?.limit ?? 100
    const offset = options?.offset ?? 0

    const buildAndFetch = async (paramName?: string, value?: string): Promise<NftItemsPage> => {
      const url = this.buildUrl(this.v3BaseUrl, "/nft/items")
      if (paramName && value) {
        url.searchParams.append(paramName, value)
      }
      url.searchParams.append("limit", limit.toString())
      url.searchParams.append("offset", offset.toString())
      if (sortByLastTransactionLt) {
        url.searchParams.append("sort_by_last_transaction_lt", "true")
      }

      const response = await this.request<NftItemsResponse>(url, "Failed to fetch NFTs")
      return {
        items: response.nft_items.map(item => attachNftItemMetadata(item, response.metadata)),
        rawItemCount: response.nft_items.length,
      }
    }

    const mergePages = (pages: readonly NftItemsPage[]): NftItemsPage => {
      return {
        items: this.dedupNftItems(pages.flatMap(page => page.items)),
        rawItemCount: pages.reduce((count, page) => count + page.rawItemCount, 0),
      }
    }

    if (addresses && addresses.length > 0) {
      const results = await Promise.all(addresses.map(async addr => buildAndFetch("address", addr)))
      return mergePages(results)
    }

    if (ownerAddresses && ownerAddresses.length > 0) {
      const results = await Promise.all(
        ownerAddresses.map(async owner => buildAndFetch("owner_address", owner)),
      )
      return mergePages(results)
    }

    if (collectionAddresses && collectionAddresses.length > 0) {
      const results = await Promise.all(
        collectionAddresses.map(async collection =>
          buildAndFetch("collection_address", collection),
        ),
      )
      return mergePages(results)
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

  async getRegisteredCompilerAbis(
    codeHashes: readonly string[],
  ): Promise<Record<string, ExtendedContractABI | null>> {
    const uniqueCodeHashes = [...new Set(codeHashes.filter(Boolean))]
    if (uniqueCodeHashes.length === 0) {
      return {}
    }

    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_getCompilerAbi")
    for (const codeHash of uniqueCodeHashes) {
      url.searchParams.append("code_hash", codeHash)
    }
    return this.request<Record<string, ExtendedContractABI | null>>(
      url,
      "Failed to fetch registered compiler ABI",
    )
  }

  async registerCompilerAbis(
    entries: readonly {
      readonly abi: ExtendedContractABI
    }[],
  ): Promise<void> {
    if (entries.length === 0) {
      return
    }
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_registerCompilerAbis")
    await this.request<null>(url, "Failed to register compiler ABI", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({
        entries: entries.map(entry => ({
          abi: entry.abi,
        })),
      }),
    })
  }

  async listRegisteredCompilerAbis(): Promise<
    readonly {
      readonly codeHash: string
      readonly abi: ExtendedContractABI
      readonly savedAt: number
    }[]
  > {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_listCompilerAbis")
    return this.request(url, "Failed to list registered compiler ABI")
  }

  async deleteRegisteredCompilerAbi(codeHash: string): Promise<void> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_deleteCompilerAbi")
    await this.request<null>(url, "Failed to delete compiler ABI", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({code_hash: codeHash}),
    })
  }

  async getRegisteredVerifiedSource(options: {
    readonly address?: string
    readonly codeHash?: string
  }): Promise<VerificationSourceResponse> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_getRegisteredVerifiedSource")
    if (options.address) {
      url.searchParams.append("address", options.address)
    }
    if (options.codeHash) {
      url.searchParams.append("code_hash", options.codeHash)
    }
    return this.request<VerificationSourceResponse>(
      url,
      "Failed to fetch registered verified source",
    )
  }

  async registerVerifiedSources(
    entries: readonly {
      readonly codeHash: string
      readonly source: VerificationSourceResponse
    }[],
  ): Promise<void> {
    if (entries.length === 0) {
      return
    }
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_registerVerifiedSources")
    await this.request<null>(url, "Failed to register verified source", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({
        entries: entries.map(entry => ({
          code_hash: entry.codeHash,
          source: entry.source,
        })),
      }),
    })
  }

  async listRegisteredVerifiedSources(): Promise<
    readonly {
      readonly artifactId: string
      readonly codeHash: string
      readonly source: VerificationSourceResponse
      readonly savedAt: number
    }[]
  > {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_listVerifiedSources")
    return this.request(url, "Failed to list registered verified sources")
  }

  async deleteRegisteredVerifiedSource(codeHash: string): Promise<void> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_deleteVerifiedSource")
    await this.request<null>(url, "Failed to delete verified source", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({code_hash: codeHash}),
    })
  }

  async deleteRegisteredVerifiedSourceArtifact(artifactId: string): Promise<void> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_deleteVerifiedSourceArtifact")
    await this.request<null>(url, "Failed to delete source artifact", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({artifact_id: artifactId}),
    })
  }

  async buildSourceTrace(
    payload: BuildSourceTraceRequest,
  ): Promise<SourceTraceResponse | undefined> {
    if (!this.localnetControlEnabled) {
      return undefined
    }

    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_buildSourceTrace")
    return this.request<SourceTraceResponse>(url, "Failed to build source trace", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify(payload),
    })
  }

  async getNodeInfo(): Promise<LocalnetNodeInfo> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_nodeInfo")
    return this.request(url, "Failed to fetch node info")
  }

  async downloadState(): Promise<Blob> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_dumpState")
    return this.requestBlob(url, "Failed to download localnet state")
  }

  async loadState(state: Blob): Promise<void> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_loadState")
    await this.request<null>(url, "Failed to load localnet state", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: state,
    })
  }

  async createCheckpoint(name: string, force = false): Promise<LocalnetCheckpoint> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_createCheckpoint")
    return this.request(url, "Failed to create checkpoint", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({name, force}),
    })
  }

  async listCheckpoints(): Promise<readonly LocalnetCheckpoint[]> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_listCheckpoints")
    return this.request(url, "Failed to list checkpoints")
  }

  async restoreCheckpoint(name: string): Promise<LocalnetCheckpoint> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_restoreCheckpoint")
    return this.request(url, "Failed to restore checkpoint", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({name}),
    })
  }

  async deleteCheckpoint(name: string): Promise<LocalnetCheckpoint> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_deleteCheckpoint")
    return this.request(url, "Failed to delete checkpoint", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({name}),
    })
  }

  async clearCheckpoints(): Promise<number> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_clearCheckpoints")
    const result = await this.request<{readonly deleted: number}>(
      url,
      "Failed to clear checkpoints",
      {
        method: "POST",
        headers: {"Content-Type": "application/json"},
        body: "{}",
      },
    )
    return result.deleted
  }

  async downloadCheckpoint(name: string): Promise<Blob> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_exportCheckpoint")
    url.searchParams.set("name", name)
    return this.requestBlob(url, "Failed to download checkpoint")
  }

  async importCheckpoint(name: string, state: Blob, force = false): Promise<LocalnetCheckpoint> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_importCheckpoint")
    url.searchParams.set("name", name)
    url.searchParams.set("force", force.toString())
    return this.request(url, "Failed to import checkpoint", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: state,
    })
  }

  async mineBlocks(blocks = 1): Promise<LocalnetMineResult> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_mine")
    return this.request(url, "Failed to mine localnet block", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({blocks}),
    })
  }

  async setMiningMode(skipEmptyBlocks: boolean): Promise<LocalnetMiningMode> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_setMiningMode")
    return this.request(url, "Failed to update localnet mining settings", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({skip_empty_blocks: skipEmptyBlocks}),
    })
  }

  async setNetworkConditions(responseDelayMs: number): Promise<LocalnetNetworkConditions> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_setNetworkConditions")
    return this.request(url, "Failed to update localnet network conditions", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({response_delay_ms: responseDelayMs}),
    })
  }

  async increaseTime(seconds: number): Promise<LocalnetTimeInfo> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_increaseTime")
    return this.request(url, "Failed to advance node time", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({seconds}),
    })
  }

  async listContracts(): Promise<readonly LocalnetContract[]> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_listContracts")
    return this.request(url, "Failed to fetch contracts")
  }

  async registerContract(address: string, name?: string): Promise<LocalnetContract> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_registerContract")
    return this.request(url, "Failed to add contract", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({address, ...(name ? {name} : {})}),
    })
  }

  async deleteContract(address: string): Promise<void> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_deleteContract")
    await this.request(url, "Failed to remove contract from Studio", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({address}),
    })
  }

  async setAddressName(address: string, name: string): Promise<void> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_setAddressName")
    await this.request<null>(url, "Failed to set address name", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({address, name}),
    })
  }

  async fundAccount(address: string, amount: bigint): Promise<string> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_fundAccount")
    const response = await this.request<FaucetResponse>(url, "Failed to fund account", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: `{"address":${JSON.stringify(address)},"amount":${amount.toString()}}`,
    })

    if (response.ok === false || response.success === false) {
      throw new Error(response.error || "Failed to fund account")
    }
    if (!response.hash) {
      throw new Error(response.error || "Faucet response did not include a message hash")
    }
    return response.hash
  }

  async fundJetton(address: string, jettonMaster: string, amount: string): Promise<string> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_fundJetton")
    const response = await this.request<SendInternalMessageResponse>(url, "Failed to fund jetton", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({address, jetton_master: jettonMaster, amount}),
    })
    return response.hash
  }

  async setShardAccount(address: string, shardAccount: string): Promise<void> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_setShardAccount")
    await this.request<null>(url, "Failed to set shard account", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({address, shard_account: shardAccount}),
    })
  }

  async setConfig(config: string): Promise<LocalnetSetConfigResult> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_setConfig")
    return this.request(url, "Failed to set localnet blockchain config", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({config}),
    })
  }

  async sendInternalMessage(boc: string): Promise<string> {
    const url = this.buildUrl(this.addressNameBaseUrl, "/acton_sendInternalMessage")
    const response = await this.request<SendInternalMessageResponse>(
      url,
      "Failed to send internal message",
      {
        method: "POST",
        headers: {"Content-Type": "application/json"},
        body: JSON.stringify({boc}),
      },
    )
    return response.hash
  }

  async sendExternalMessage(boc: string): Promise<string> {
    const url = this.buildUrl(this.v2BaseUrl, "/sendBocReturnHash")
    const response = await this.request<SendExternalMessageResponse>(
      url,
      "Failed to send external message",
      {
        method: "POST",
        headers: {"Content-Type": "application/json"},
        body: JSON.stringify({boc}),
      },
    )
    return response.hash
  }

  getEndpoints(): {
    readonly apiV2: string
    readonly apiV3: string
    readonly admin: string
  } {
    return {
      apiV2: this.buildUrl(this.v2BaseUrl, "").toString().replace(/\/$/, ""),
      apiV3: this.buildUrl(this.v3BaseUrl, "").toString().replace(/\/$/, ""),
      admin: this.buildUrl(this.addressNameBaseUrl, "").toString().replace(/\/$/, ""),
    }
  }

  usesToncenterApiEndpoint(): boolean {
    return this.toncenterApiCompatible
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

  private async readAccountHistoryStream(
    address: string,
    handlers: AccountHistoryStreamHandlers,
    signal: AbortSignal,
  ): Promise<void> {
    try {
      const url = this.buildStreamingSseUrl()
      const response = await fetch(
        url.toString(),
        this.withRequestHeaders(url, {
          method: "POST",
          headers: {
            Accept: "text/event-stream",
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            addresses: [address],
            types: handlers.onActions ? ["transactions", "actions"] : ["transactions"],
            min_finality: "confirmed",
          }),
          signal,
        }),
      )

      if (!response.ok) {
        if (response.status === 401) {
          this.onUnauthorized?.()
        }
        const body = await response.text().catch(() => "")
        throw new Error(body || `Streaming subscription failed with status ${response.status}`)
      }
      if (!response.body) {
        throw new Error("Streaming subscription returned an empty body")
      }

      await this.readSseEvents(response.body, async value => {
        if (isStreamingTransactionsEvent(value)) {
          handlers.onTransactions(value)
        } else if (isStreamingActionsEvent(value)) {
          const metadata = value.metadata
            ? await this.enrichActionMetadata(value.actions, value.metadata)
            : value.metadata
          handlers.onActions?.(metadata === value.metadata ? value : {...value, metadata})
        }
      })
    } catch (error) {
      if (signal.aborted) return
      handlers.onError?.(error instanceof Error ? error : new Error(String(error)))
    }
  }

  private async readSseEvents(
    body: ReadableStream<Uint8Array>,
    onEvent: (value: unknown) => void | Promise<void>,
  ): Promise<void> {
    const reader = body.getReader()
    const decoder = new TextDecoder()
    let buffer = ""
    let dataLines: string[] = []

    const dispatch = async (): Promise<void> => {
      if (dataLines.length === 0) {
        return
      }

      const data = dataLines.join("\n")
      dataLines = []
      try {
        await onEvent(JSON.parse(data) as unknown)
      } catch (error) {
        console.debug("Failed to parse streaming event", error)
      }
    }

    const processLine = async (line: string): Promise<void> => {
      if (line.length === 0) {
        await dispatch()
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
        await processLine(line)
      }
    }

    if (buffer.length > 0) {
      await processLine(buffer)
    }
    await dispatch()
  }

  private attachJettonWalletMaster(
    wallet: JettonWallet,
    metadata: JettonWalletMetadata | undefined,
  ): JettonWallet {
    const master = wallet.master ?? jettonMasterMetadataFromWalletResponse(wallet.jetton, metadata)
    return master ? {...wallet, master} : wallet
  }

  private async enrichActionMetadata(
    actions: readonly V3Action[],
    metadata: V3Metadata,
  ): Promise<V3Metadata> {
    const missingMasterAddresses = collectActionAssetAddresses(actions).filter(address => {
      const tokenInfo = metadataTokenInfoForAddress(metadata, address)
      return tokenInfo !== undefined && !metadataTokenHasSymbol(tokenInfo)
    })
    if (missingMasterAddresses.length === 0) {
      return metadata
    }

    try {
      const masters = await this.getJettonMasters(missingMasterAddresses)
      return mergeJettonMastersIntoMetadata(metadata, masters)
    } catch (error) {
      console.debug("Failed to enrich action jetton metadata", error)
      return metadata
    }
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
    const response = await fetch(url.toString(), this.withRequestHeaders(url, options))
    if (response.status === 401) {
      this.onUnauthorized?.()
    }
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

  private async requestDnsFromChain(domain: string): Promise<DnsResolvedResponse> {
    const url = this.buildUrl(this.v2BaseUrl, "/dnsResolve")
    url.searchParams.append("address", TON_DNS_ROOT_ADDRESS)
    url.searchParams.append("name", domain)
    url.searchParams.append("category", "wallet")
    url.searchParams.append("ttl", DNS_RESOLVE_TTL.toString())

    return this.request<DnsResolvedResponse>(url, "Failed to resolve TON DNS name on-chain")
  }

  private async requestBlob(url: URL, errorMessage: string): Promise<Blob> {
    const response = await fetch(url.toString(), this.withRequestHeaders(url))
    if (response.status === 401) {
      this.onUnauthorized?.()
    }
    if (!response.ok) {
      const text = await response.text()
      let error = text
      try {
        error = this.extractError(JSON.parse(text) as unknown) ?? text
      } catch {
        // Preserve a non-JSON server response when one is available.
      }
      throw new Error(error || errorMessage)
    }
    return response.blob()
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

  private withRequestHeaders(url: URL, options?: RequestInit): RequestInit | undefined {
    const headers = new Headers(options?.headers)
    let changed = false

    const isLocalnetApiUrl = this.isLocalnetApiUrl(url)
    if (isLocalnetApiUrl) {
      headers.set(REQUEST_SOURCE_HEADER, STUDIO_UI_REQUEST_SOURCE)
      changed = true
    }

    if (this.localnetApiToken && isLocalnetApiUrl) {
      headers.set("Authorization", `Bearer ${this.localnetApiToken}`)
      changed = true
    }

    if (this.toncenterApiKey && this.isToncenterApiUrl(url)) {
      headers.set("X-API-Key", this.toncenterApiKey)
      changed = true
    }

    return changed ? {...options, headers} : options
  }

  private isToncenterApiUrl(url: URL): boolean {
    return (
      this.isUrlWithinBase(url, this.buildUrl(this.v2BaseUrl, "")) ||
      this.isUrlWithinBase(url, this.buildUrl(this.v3BaseUrl, "")) ||
      this.isUrlWithinBase(url, this.buildStreamingSseUrl())
    )
  }

  private isLocalnetApiUrl(url: URL): boolean {
    return this.isUrlWithinBase(url, this.buildUrl(this.addressNameBaseUrl, ""))
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

function appendOptionalSearchParam(
  url: URL,
  name: string,
  value: string | number | undefined,
): void {
  if (value !== undefined) {
    url.searchParams.append(name, value.toString())
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

function isStreamingActionsEvent(value: unknown): value is StreamingActionsEvent {
  if (typeof value !== "object" || value === null) {
    return false
  }

  const event = value as Partial<StreamingActionsEvent>
  return (
    event.type === "actions" &&
    (event.finality === "pending" ||
      event.finality === "confirmed" ||
      event.finality === "finalized") &&
    Array.isArray(event.actions)
  )
}

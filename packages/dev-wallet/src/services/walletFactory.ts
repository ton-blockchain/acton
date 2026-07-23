import {mnemonicNew, mnemonicValidate} from "@ton/crypto"
import {invoke} from "@tauri-apps/api/core"
import {
  ApiClientToncenter,
  Network,
  Signer,
  WalletV4R2Adapter,
  WalletV5R1Adapter,
} from "@ton/walletkit"

import {
  WALLET_NETWORKS,
  type WalletNetworkId,
  type WalletRecord,
  type WalletVersion,
} from "../domain/wallet"
import {isTauriRuntime} from "./walletVault"

const BALANCE_CACHE_TTL_MS = 10_000
const balanceRequests = new Map<
  string,
  {readonly expiresAt: number; readonly request: Promise<string>}
>()

export interface CreateWalletInput {
  readonly name: string
  readonly mnemonic: readonly string[]
  readonly network: WalletNetworkId
  readonly version: WalletVersion
}

export async function createRandomMnemonic(): Promise<readonly string[]> {
  return mnemonicNew(24)
}

export function normalizeMnemonic(value: string): readonly string[] {
  return value.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean)
}

export async function validateMnemonic(words: readonly string[]): Promise<boolean> {
  return words.length === 24 && mnemonicValidate([...words])
}

export async function deriveWalletRecord(input: CreateWalletInput): Promise<WalletRecord> {
  const signer = await Signer.fromMnemonic([...input.mnemonic], {type: "ton"})
  const networkDefinition = WALLET_NETWORKS[input.network]
  const network = input.network === "mainnet" ? Network.mainnet() : Network.testnet()
  const client = new ApiClientToncenter({
    endpoint: networkDefinition.endpoint,
    network,
  })
  const adapter =
    input.version === "v4r2"
      ? await WalletV4R2Adapter.create(signer, {client, network})
      : await WalletV5R1Adapter.create(signer, {client, network})

  return {
    id: crypto.randomUUID(),
    name: input.name.trim(),
    address: adapter.getAddress({testnet: input.network === "testnet"}),
    publicKey: signer.publicKey,
    version: input.version,
    network: input.network,
    createdAt: new Date().toISOString(),
  }
}

export async function fetchWalletBalance(wallet: WalletRecord): Promise<string> {
  const cacheKey = `${wallet.network}:${wallet.address}`
  const cached = balanceRequests.get(cacheKey)
  if (cached && cached.expiresAt > Date.now()) {
    return cached.request
  }

  const request = (
    isTauriRuntime()
      ? invoke<{balance: string}>("get_wallet_balance", {
          request: {walletId: wallet.id},
        }).then(result => result.balance)
      : new ApiClientToncenter({
          endpoint: WALLET_NETWORKS[wallet.network].endpoint,
          network: wallet.network === "mainnet" ? Network.mainnet() : Network.testnet(),
        }).getBalance(wallet.address)
  ).catch(error => {
    balanceRequests.delete(cacheKey)
    throw error
  })
  balanceRequests.set(cacheKey, {
    expiresAt: Date.now() + BALANCE_CACHE_TTL_MS,
    request,
  })
  return request
}

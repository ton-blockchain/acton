import {invoke} from "@tauri-apps/api/core"

import {WALLET_NETWORKS, type WalletRecord} from "../domain/wallet"
import {isTauriRuntime} from "./walletVault"

export type WalletActivityDirection = "incoming" | "outgoing" | "contract"

export interface WalletActivityItem {
  readonly hash: string
  readonly timestamp: number
  readonly direction: WalletActivityDirection
  readonly valueNano: string
  readonly feeNano: string
  readonly counterparty?: string
}

interface ToncenterEnvelope {
  readonly ok: boolean
  readonly result?: readonly unknown[]
  readonly error?: string
}

const ACTIVITY_CACHE_TTL_MS = 15_000
const activityRequests = new Map<
  string,
  {readonly expiresAt: number; readonly request: Promise<readonly WalletActivityItem[]>}
>()

export function fetchWalletActivity(
  wallet: WalletRecord,
  force = false,
): Promise<readonly WalletActivityItem[]> {
  const cacheKey = `${wallet.network}:${wallet.address}`
  const cached = activityRequests.get(cacheKey)
  if (!force && cached && cached.expiresAt > Date.now()) {
    return cached.request
  }

  const request = loadWalletActivity(wallet).catch(error => {
    activityRequests.delete(cacheKey)
    throw error
  })
  activityRequests.set(cacheKey, {
    expiresAt: Date.now() + ACTIVITY_CACHE_TTL_MS,
    request,
  })
  return request
}

async function loadWalletActivity(wallet: WalletRecord): Promise<readonly WalletActivityItem[]> {
  if (isTauriRuntime()) {
    return await invoke<readonly WalletActivityItem[]>("get_wallet_activity", {
      request: {walletId: wallet.id, limit: 10},
    })
  }

  const url = new URL(`${WALLET_NETWORKS[wallet.network].endpoint}/api/v2/getTransactions`)
  url.searchParams.set("address", wallet.address)
  url.searchParams.set("limit", "10")
  url.searchParams.set("archival", "true")
  const response = await fetch(url)
  if (response.status === 429) {
    throw new Error("Toncenter rate limit reached. Try again shortly.")
  }
  if (!response.ok) {
    throw new Error(`Activity is unavailable (HTTP ${response.status}).`)
  }
  const envelope = (await response.json()) as ToncenterEnvelope
  if (!envelope.ok) {
    throw new Error(envelope.error ?? "Activity is temporarily unavailable.")
  }
  return (envelope.result ?? []).flatMap(mapToncenterTransaction)
}

function mapToncenterTransaction(value: unknown): readonly WalletActivityItem[] {
  if (!isRecord(value)) return []
  const transactionId = isRecord(value.transaction_id) ? value.transaction_id : undefined
  const hash = stringValue(transactionId?.hash)
  if (!hash) return []
  const incoming = isRecord(value.in_msg) ? value.in_msg : undefined
  const outgoing = Array.isArray(value.out_msgs) ? value.out_msgs.filter(isRecord) : []

  if (outgoing.length > 0) {
    const valueNano = outgoing
      .map(message => stringValue(message.value))
      .filter((amount): amount is string => amount !== undefined)
      .reduce((total, amount) => total + parseNano(amount), 0n)
      .toString()
    return [
      {
        hash,
        timestamp: numberValue(value.utime),
        direction: "outgoing",
        valueNano,
        feeNano: stringValue(value.fee) ?? "0",
        counterparty: nonEmptyString(outgoing[0]?.destination),
      },
    ]
  }

  const valueNano = stringValue(incoming?.value) ?? "0"
  return [
    {
      hash,
      timestamp: numberValue(value.utime),
      direction: parseNano(valueNano) > 0n ? "incoming" : "contract",
      valueNano,
      feeNano: stringValue(value.fee) ?? "0",
      counterparty: nonEmptyString(incoming?.source),
    },
  ]
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function stringValue(value: unknown): string | undefined {
  if (typeof value === "string") return value
  if (typeof value === "number" && Number.isFinite(value)) return String(value)
  return undefined
}

function nonEmptyString(value: unknown): string | undefined {
  const text = stringValue(value)?.trim()
  return text || undefined
}

function numberValue(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0
}

function parseNano(value: string): bigint {
  try {
    return BigInt(value)
  } catch {
    return 0n
  }
}

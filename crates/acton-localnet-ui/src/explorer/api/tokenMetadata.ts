import {addressKey} from "./compilerAbi"
import type {AccountStateTokenInfo, V3Metadata} from "./types"

export function getMetadataTokenInfo(
  metadata: V3Metadata,
  address: string,
  type: string,
): AccountStateTokenInfo | undefined {
  const entries = metadata[address]?.token_info ?? metadata[addressKey(address)]?.token_info ?? []
  return entries.find(info => info.type === type)
}

export function metadataTokenString(
  tokenInfo: AccountStateTokenInfo | undefined,
  key: string,
): string | undefined {
  const value = tokenInfo?.[key]
  if (isNonEmptyString(value)) {
    return value
  }

  const extra = isRecord(tokenInfo?.extra) ? tokenInfo.extra : undefined
  const extraValue = extra?.[key]
  return isNonEmptyString(extraValue) ? extraValue : undefined
}

export function metadataTokenDecimals(
  tokenInfo: AccountStateTokenInfo | undefined,
): number | undefined {
  const rawDecimals = metadataTokenString(tokenInfo, "decimals")
  if (!rawDecimals) {
    return undefined
  }

  const decimals = Number(rawDecimals)
  return Number.isInteger(decimals) && decimals >= 0 && decimals <= 36 ? decimals : undefined
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

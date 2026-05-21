import type {JettonMaster, V3TransactionListItem} from "../explorer/api/types"

export function parseTonAmount(value: string): number | undefined {
  const trimmed = value.trim()
  if (!trimmed || !/^\d+(\.\d{0,9})?$/.test(trimmed)) {
    return undefined
  }

  const [wholePart, fractionPart = ""] = trimmed.split(".")
  const whole = BigInt(wholePart)
  const fraction = BigInt(fractionPart.padEnd(9, "0"))
  const nano = whole * 1_000_000_000n + fraction
  if (nano <= 0n || nano > BigInt(Number.MAX_SAFE_INTEGER)) {
    return undefined
  }
  return Number(nano)
}

export function contentString(
  content: Record<string, unknown> | undefined,
  key: string,
): string | undefined {
  const value = content?.[key]
  return typeof value === "string" && value.length > 0 ? value : undefined
}

export function formatTokenSupply(token: JettonMaster): string {
  const decimals = Number(token.jetton_content.decimals || 9)
  return (Number(token.total_supply) / 10 ** decimals).toLocaleString()
}

export function shortHash(hash: string): string {
  return hash.length > 16 ? `${hash.slice(0, 8)}…${hash.slice(-8)}` : hash
}

export function matchesQuery(fields: readonly (string | undefined)[], query: string): boolean {
  return fields.some(field => field?.toLocaleLowerCase().includes(query))
}

export function collectRecentAccounts(transactions: readonly V3TransactionListItem[]): string[] {
  const seen = new Set<string>()
  const accounts: string[] = []

  for (const transaction of transactions) {
    if (!seen.has(transaction.account)) {
      seen.add(transaction.account)
      accounts.push(transaction.account)
    }
    if (accounts.length === 6) {
      break
    }
  }

  return accounts
}

export function isTextEntryTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false
  }

  const tagName = target.tagName.toLowerCase()
  return tagName === "input" || tagName === "textarea" || target.isContentEditable
}

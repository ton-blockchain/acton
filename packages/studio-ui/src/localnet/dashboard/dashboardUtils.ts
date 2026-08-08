import type {V3TransactionListItem} from "@acton/explorer-core/api/types"

export function contentString(
  content: Record<string, unknown> | undefined,
  key: string,
): string | undefined {
  const value = content?.[key]
  return typeof value === "string" && value.length > 0 ? value : undefined
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

export function formatForkNetworkLabel(forkNetwork?: string | null): string | undefined {
  const normalizedForkNetwork = forkNetwork?.trim()
  if (!normalizedForkNetwork) {
    return undefined
  }

  return `${normalizedForkNetwork.toLocaleLowerCase()} fork`
}

export function isTextEntryTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false
  }

  const tagName = target.tagName.toLowerCase()
  return tagName === "input" || tagName === "textarea" || target.isContentEditable
}

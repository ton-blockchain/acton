import {formatDnsName} from "./utils"

export interface AccountNameDetailValue {
  readonly copyValue: string
  readonly displayValue: string
}

export interface AccountNameDetailGroup {
  readonly key: "custom" | "registry" | "ton-dns" | "telegram"
  readonly label: "Custom" | "Known names" | "TON DNS" | "Telegram"
  readonly values: readonly AccountNameDetailValue[]
}

interface AccountNameDetailsOptions {
  readonly displayName?: string
  readonly domain?: string
  readonly domains?: readonly string[]
  readonly customName?: string
  readonly registryName?: string
  readonly tonDnsName?: string
}

export interface AccountNameDetails {
  readonly displayNameText?: string
  readonly groups: readonly AccountNameDetailGroup[]
}

export function getAccountNameDetails({
  displayName,
  domain,
  domains = [],
  customName,
  registryName,
  tonDnsName,
}: AccountNameDetailsOptions): AccountNameDetails {
  const normalizedDisplayName = displayName?.trim()
  const tonDnsNames = uniqueNames([domain, ...domains, tonDnsName])
  const displayNameText =
    normalizedDisplayName && includesName(tonDnsNames, normalizedDisplayName)
      ? formatDnsName(normalizedDisplayName)
      : displayName
  const groups = [
    nameDetailGroup("custom", "Custom", [customName], normalizedDisplayName),
    nameDetailGroup("registry", "Known names", [registryName], normalizedDisplayName),
    nameDetailGroup(
      "ton-dns",
      "TON DNS",
      tonDnsNames.filter(name => !isTelegramDnsName(name)),
      normalizedDisplayName,
      true,
    ),
    nameDetailGroup(
      "telegram",
      "Telegram",
      tonDnsNames.filter(isTelegramDnsName),
      normalizedDisplayName,
      true,
    ),
  ].filter((group): group is AccountNameDetailGroup => group !== undefined)

  return {displayNameText, groups}
}

function nameDetailGroup(
  key: AccountNameDetailGroup["key"],
  label: AccountNameDetailGroup["label"],
  values: readonly (string | undefined)[],
  excludedValue: string | undefined,
  formatDnsNames = false,
): AccountNameDetailGroup | undefined {
  const excludedKey = excludedValue?.toLowerCase()
  const filteredValues = uniqueNames(values).filter(value => value.toLowerCase() !== excludedKey)
  return filteredValues.length > 0
    ? {
        key,
        label,
        values: filteredValues.map(value => ({
          copyValue: value,
          displayValue: formatDnsNames ? formatDnsName(value) : value,
        })),
      }
    : undefined
}

function uniqueNames(names: readonly (string | undefined)[]): readonly string[] {
  const unique = new Map<string, string>()
  for (const name of names) {
    const normalizedName = name?.trim()
    const nameKey = normalizedName?.toLowerCase()
    if (normalizedName && nameKey && !unique.has(nameKey)) {
      unique.set(nameKey, normalizedName)
    }
  }
  return [...unique.values()]
}

function includesName(names: readonly string[], expectedName: string): boolean {
  const normalizedExpectedName = expectedName.toLowerCase()
  return names.some(name => name.toLowerCase() === normalizedExpectedName)
}

function isTelegramDnsName(name: string): boolean {
  return name.toLowerCase().endsWith(".t.me")
}

//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

export type TonExplorer = "tonscan" | "tonviewer"

export interface TonAddressMatch {
  readonly address: string
  readonly startIndex: number
  readonly length: number
  readonly isTestnet: boolean
}

const MAX_LINKABLE_LINE_LENGTH = 300
const RAW_ADDRESS_PATTERN = /(^|[^\w-])(-?\d:[\dA-Fa-f]{64})\b/g
const USER_FRIENDLY_ADDRESS_PATTERN = /\b[0EUfk][Qf][\w+-]{46}\b/g

export function findTonAddressMatches(line: string): TonAddressMatch[] {
  if (line.length > MAX_LINKABLE_LINE_LENGTH) {
    return []
  }

  if (!line.includes(":") && !line.includes("Q") && !line.includes("f")) {
    return []
  }

  const matches: TonAddressMatch[] = []
  for (const match of line.matchAll(RAW_ADDRESS_PATTERN)) {
    const address = match[2]
    const prefixLength = match[1].length
    matches.push({
      address,
      startIndex: match.index + prefixLength,
      length: address.length,
      isTestnet: false,
    })
  }

  for (const match of line.matchAll(USER_FRIENDLY_ADDRESS_PATTERN)) {
    matches.push({
      address: match[0],
      startIndex: match.index,
      length: match[0].length,
      isTestnet: isTestnetUserFriendlyAddress(match[0]),
    })
  }

  return matches.sort((left, right) => left.startIndex - right.startIndex)
}

export function normalizeTonExplorer(value: string | undefined): TonExplorer {
  return value === "tonviewer" ? "tonviewer" : "tonscan"
}

export function createTonAddressExplorerUrl(
  address: string,
  explorer: TonExplorer,
  isTestnet: boolean,
): string {
  const encodedAddress = encodeURIComponent(address)
  if (explorer === "tonviewer") {
    const domain = isTestnet ? "testnet.tonviewer.com" : "tonviewer.com"
    return `https://${domain}/${encodedAddress}`
  }

  const domain = isTestnet ? "testnet.tonscan.org" : "tonscan.org"
  return `https://${domain}/address/${encodedAddress}`
}

function isTestnetUserFriendlyAddress(address: string): boolean {
  return address.startsWith("k") || address.startsWith("0")
}

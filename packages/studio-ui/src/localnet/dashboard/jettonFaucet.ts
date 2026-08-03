export function parseJettonAmount(value: string, decimals: number): bigint | undefined {
  const trimmed = value.trim()
  if (!trimmed) return undefined

  const amountPattern =
    decimals === 0 ? /^(\d+)$/ : new RegExp(`^(\\d+)(?:\\.(\\d{0,${decimals}}))?$`)
  const match = trimmed.match(amountPattern)
  if (!match) return undefined

  const [, wholePart, fractionPart = ""] = match
  const scale = 10n ** BigInt(decimals)
  const fraction = decimals === 0 ? 0n : BigInt(fractionPart.padEnd(decimals, "0"))
  const amount = BigInt(wholePart) * scale + fraction
  return amount > 0n ? amount : undefined
}

export function normalizeJettonDecimals(value: unknown): number {
  if (typeof value !== "string" || !/^\d+$/.test(value)) return 9

  const decimals = Number(value)
  if (!Number.isSafeInteger(decimals) || decimals < 0 || decimals > 30) return 9
  return decimals
}

import type React from "react"
import {Address} from "@ton/core"
export {formatCurrency} from "@acton/ui"

export function formatAddress(address: string): string {
  if (!address) return "unknown"
  try {
    const parsed = Address.parse(address)
    const displayAddress = parsed.toString({testOnly: true})
    return `${displayAddress.slice(0, 6)}...${displayAddress.slice(-6)}`
  } catch {
    if (address.length <= 12) return address
    return `${address.slice(0, 6)}...${address.slice(Math.max(0, address.length - 6))}`
  }
}

export function truncateMiddle(value: string, maxLength: number): string {
  if (value.length <= maxLength) return value
  if (maxLength <= 0) return ""
  if (maxLength === 1) return "…"

  const visibleLength = maxLength - 1
  const startLength = Math.ceil(visibleLength / 2)
  const endLength = Math.floor(visibleLength / 2)
  const end = endLength === 0 ? "" : value.slice(-endLength)
  return `${value.slice(0, startLength)}…${end}`
}

export function formatDecimalAmount(value: string, decimals: number): string {
  if (!/^[0-9]+$/.test(value)) {
    return value
  }

  try {
    const raw = BigInt(value)
    const divisor = 10n ** BigInt(decimals)
    const whole = raw / divisor
    const fraction = raw % divisor
    if (decimals === 0 || fraction === 0n) {
      return whole.toString()
    }

    const fractionText = fraction.toString().padStart(decimals, "0").replace(/0+$/, "")
    return `${whole}.${fractionText}`
  } catch {
    return value
  }
}

export const formatNumber = (v: number | bigint | undefined | null): React.JSX.Element => {
  if (v === undefined || v === null) return <span>—</span>
  return <span className="number-value">{v.toString()}</span>
}

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

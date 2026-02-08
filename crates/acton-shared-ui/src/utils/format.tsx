import type React from "react"
import {Address} from "@ton/core"

export const formatCurrency = (value: bigint | undefined): string => {
  if (value === undefined || value === 0n) return "0 TON"
  const numberValue = Number(value)
  const displayValue = numberValue / 1_000_000_000
  const formatted = displayValue
    .toFixed(9)
    .replace(/(\.\d*[1-9])0+$/, "$1")
    .replace(/\.0+$/, "")
  return `${formatted} TON`
}

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

export const formatNumber = (v: number | bigint | undefined | null): React.JSX.Element => {
  if (v === undefined || v === null) return <span>—</span>
  return <span className="number-value">{v.toString()}</span>
}

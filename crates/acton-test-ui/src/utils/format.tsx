import type { Address, ExternalAddress } from "@ton/core"
import type React from "react"

export const formatCurrency = (value: bigint | undefined): string => {
  if (value === undefined || value === 0n) return "0 TON"
  const numValue = Number(value)
  const displayValue = numValue / 1_000_000_000
  const formatted = displayValue
    .toFixed(9)
    .replace(/(\.\d*[1-9])0+$/, "$1")
    .replace(/\.0+$/, "")
  return `${formatted} TON`
}

export function formatAddress(address: string): string {
  if (address.length <= 12) return address
  return `${address.slice(0, 6)}...${address.slice(Math.max(0, address.length - 6))}`
}

export const formatNumber = (v: number | bigint | undefined | null): React.JSX.Element => {
  if (v === undefined || v === null) return <span>—</span>
  return <span className="number-value">{v.toString()}</span>
}

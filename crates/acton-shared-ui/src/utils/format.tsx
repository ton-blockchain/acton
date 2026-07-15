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

export const formatNumber = (v: number | bigint | undefined | null): React.JSX.Element => {
  if (v === undefined || v === null) return <span>—</span>
  return <span className="number-value">{v.toString()}</span>
}

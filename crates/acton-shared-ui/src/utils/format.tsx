import {Address} from "@ton/core"
import type React from "react"

const NANOGRAM_DECIMALS = 9

export const formatCurrency = (value: bigint | undefined): string => {
  if (value === undefined || value === 0n) return "0 GRAM"
  const sign = value < 0n ? "-" : ""
  const digits = (value < 0n ? -value : value).toString()
  const formatted =
    digits.length <= NANOGRAM_DECIMALS
      ? trimNanogramFraction(`0.${digits.padStart(NANOGRAM_DECIMALS, "0")}`)
      : trimNanogramFraction(
          `${digits.slice(0, -NANOGRAM_DECIMALS)}.${digits.slice(-NANOGRAM_DECIMALS)}`,
        )

  return `${sign}${formatted} GRAM`
}

function trimNanogramFraction(value: string): string {
  let result = value

  while (result.endsWith("0")) {
    result = result.slice(0, -1)
  }

  return result.endsWith(".") ? result.slice(0, -1) : result
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

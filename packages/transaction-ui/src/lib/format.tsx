import {Address} from "@ton/core"
import {shortenMiddle} from "@acton/ui"

export function formatAddress(address: string): string {
  if (!address) return "unknown"
  try {
    const parsed = Address.parse(address)
    const displayAddress = parsed.toString({testOnly: true})
    return shortenMiddle(displayAddress, {start: 6, end: 6, separator: "..."})
  } catch {
    if (address.length <= 12) return address
    return shortenMiddle(address, {start: 6, end: 6, separator: "..."})
  }
}

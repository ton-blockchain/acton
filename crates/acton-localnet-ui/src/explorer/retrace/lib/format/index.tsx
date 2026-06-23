import type {Address, ExternalAddress} from "@ton/core"

export const formatCurrency = (value: bigint | undefined) => {
  if (value === undefined || value === null) return "0 TON"
  if (value === 0n) return "0 TON"

  const numValue = Number(value)
  const displayValue = numValue / 1_000_000_000

  const formatted = displayValue
    .toFixed(9)
    .replace(/(\.[0-9]*[1-9])0+$/, "$1")
    .replace(/\.0+$/, "")

  return `${formatted} TON`
}

export const formatAddress = (address: Address | ExternalAddress | string | undefined | null) => {
  if (!address) return "—"
  if (address === "external") return "External"
  return String(address)
}

export const formatBoolean = (v: boolean) => {
  return <span className={v ? "booleanTrue" : "booleanFalse"}>{v ? "Yes" : "No"}</span>
}

export const formatNumber = (v: number | bigint | undefined | null) => {
  if (v === undefined || v === null) return "—"
  return <span className="number-value">{v.toString()}</span>
}

export const shortenHash = (hash: string, startChars: number, endChars: number): string => {
  if (!hash) return "—"
  if (hash.length <= startChars + endChars + 3) {
    return hash
  }
  return `${hash.substring(0, startChars)}...${hash.substring(hash.length - endChars)}`
}

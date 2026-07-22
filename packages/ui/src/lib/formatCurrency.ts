const NANOGRAM_DECIMALS = 9

function trimNanogramFraction(value: string): string {
  let result = value

  while (result.endsWith("0")) {
    result = result.slice(0, -1)
  }

  return result.endsWith(".") ? result.slice(0, -1) : result
}

export function formatCurrency(value: bigint | undefined): string {
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

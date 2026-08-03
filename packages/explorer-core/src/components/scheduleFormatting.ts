export const SECONDS_PER_DAY = 86_400

export function formatGramAmount(value: bigint, maximumFractionDigits = 2): string {
  const fractionDigits = Math.max(0, Math.min(9, Math.trunc(maximumFractionDigits)))
  const negative = value < 0n
  const absolute = negative ? -value : value
  const roundingStep = 10n ** BigInt(9 - fractionDigits)
  const rounded =
    fractionDigits === 9 ? absolute : ((absolute + roundingStep / 2n) / roundingStep) * roundingStep
  const whole = rounded / 1_000_000_000n
  const fraction = (rounded % 1_000_000_000n)
    .toString()
    .padStart(9, "0")
    .slice(0, fractionDigits)
    .replace(/0+$/, "")
  return `${negative ? "-" : ""}${whole.toLocaleString()}${fraction ? `.${fraction}` : ""} GRAM`
}

export function formatScheduleDate(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, {
    day: "2-digit",
    month: "short",
    year: "numeric",
  }).format(new Date(timestamp * 1000))
}

export function formatSchedulePeriod(seconds: number): string {
  if (seconds % SECONDS_PER_DAY === 0) {
    const days = seconds / SECONDS_PER_DAY
    return `${days} ${days === 1 ? "day" : "days"}`
  }
  if (seconds % 3600 === 0) {
    const hours = seconds / 3600
    return `${hours} ${hours === 1 ? "hour" : "hours"}`
  }
  return `${seconds.toLocaleString()} seconds`
}

export function formatTimeUntil(timestamp: number, nowSeconds: number): string {
  const remaining = Math.max(0, timestamp - nowSeconds)
  const days = Math.ceil(remaining / SECONDS_PER_DAY)
  if (days >= 7) {
    const weeks = Math.max(1, Math.round(days / 7))
    return `in ${weeks} ${weeks === 1 ? "week" : "weeks"}`
  }
  if (days >= 1) {
    return `in ${days} ${days === 1 ? "day" : "days"}`
  }

  const hours = Math.ceil(remaining / 3600)
  if (hours >= 1) {
    return `in ${hours} ${hours === 1 ? "hour" : "hours"}`
  }
  return "soon"
}

export function capitalize<T extends string>(value: T): Capitalize<T> {
  return `${value.charAt(0).toUpperCase()}${value.slice(1)}` as Capitalize<T>
}

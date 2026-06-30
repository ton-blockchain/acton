import {formatCurrency} from "@/utils/format"

const DECIMAL_SCALAR_PATTERN = /^-?\d+(?:\.\d+)?$/
const INTEGER_SCALAR_PATTERN = /^-?\d+$/

export function isDecimalScalarValue(value: string): boolean {
  return DECIMAL_SCALAR_PATTERN.test(value)
}

export function isHexDisplayValue(value: string): boolean {
  return value.startsWith("0x") || value.startsWith("-0x")
}

function isAsciiAlphanumeric(value: string): boolean {
  return /^[A-Za-z0-9]$/.test(value)
}

function isAsciiDigit(value: string): boolean {
  return value >= "0" && value <= "9"
}

function isAsciiLowercase(value: string): boolean {
  return value >= "a" && value <= "z"
}

function isAsciiUppercase(value: string): boolean {
  return value >= "A" && value <= "Z"
}

function identifierWordBoundary(prev: string, current: string, next: string | undefined): boolean {
  if (isAsciiDigit(prev) !== isAsciiDigit(current)) {
    return true
  }

  if (isAsciiLowercase(prev) && isAsciiUppercase(current)) {
    return true
  }

  return (
    isAsciiUppercase(prev) &&
    isAsciiUppercase(current) &&
    next !== undefined &&
    isAsciiLowercase(next)
  )
}

function identifierHasWord(name: string, needle: string): boolean {
  let start: number | undefined
  let prev: string | undefined

  for (let index = 0; index < name.length; index += 1) {
    const current = name[index]
    if (!isAsciiAlphanumeric(current)) {
      if (start !== undefined && name.slice(start, index).toLowerCase() === needle.toLowerCase()) {
        return true
      }

      start = undefined
      prev = undefined
      continue
    }

    const next = index + 1 < name.length ? name[index + 1] : undefined
    if (prev !== undefined && start !== undefined && identifierWordBoundary(prev, current, next)) {
      if (name.slice(start, index).toLowerCase() === needle.toLowerCase()) {
        return true
      }
      start = index
    } else if (start === undefined) {
      start = index
    }

    prev = current
  }

  return start !== undefined && name.slice(start).toLowerCase() === needle.toLowerCase()
}

function shouldFormatIntegerAsHex(fieldName: string | undefined): boolean {
  if (fieldName === undefined) {
    return false
  }

  return (
    identifierHasWord(fieldName, "key") ||
    (identifierHasWord(fieldName, "subwallet") && identifierHasWord(fieldName, "id"))
  )
}

function shouldFormatCoins(typeName: string | undefined, fieldName: string | undefined): boolean {
  return (
    typeName === "coins" &&
    fieldName !== undefined &&
    (identifierHasWord(fieldName, "ton") ||
      identifierHasWord(fieldName, "gram") ||
      identifierHasWord(fieldName, "grams"))
  )
}

function formatIntegerAsHex(value: string): string {
  const parsedValue = BigInt(value)
  const sign = parsedValue < 0n ? "-" : ""
  const absoluteValue = parsedValue < 0n ? -parsedValue : parsedValue
  return `${sign}0x${absoluteValue.toString(16)}`
}

export function formatScalarByFieldName({
  value,
  typeName,
  fieldName,
}: {
  readonly value: string
  readonly typeName?: string
  readonly fieldName?: string
}): string {
  if (!INTEGER_SCALAR_PATTERN.test(value)) {
    return value
  }

  if (shouldFormatIntegerAsHex(fieldName)) {
    try {
      return formatIntegerAsHex(value)
    } catch {
      return value
    }
  }

  if (shouldFormatCoins(typeName, fieldName)) {
    try {
      return formatCurrency(BigInt(value))
    } catch {
      return value
    }
  }

  return value
}

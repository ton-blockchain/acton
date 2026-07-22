import {Address, ExternalAddress} from "@ton/core"

export type TonAddressKind = "internal" | "external" | "any"
export type ParsedTonAddress = Address | ExternalAddress | "none"

export const TON_ADDR_NONE = "addr_none"
export const SAMPLE_EXTERNAL_ADDRESS = "External<8:0>"

export function parseTonAddress(value: string, kind: TonAddressKind): ParsedTonAddress {
  const trimmed = value.trim()
  if (!trimmed) {
    throw new Error("Address is required.")
  }

  if (kind === "any" && (trimmed === TON_ADDR_NONE || trimmed === "none")) {
    return "none"
  }

  if (kind === "internal") {
    return Address.parse(trimmed)
  }
  if (kind === "external") {
    return parseExternalAddress(trimmed)
  }

  try {
    return Address.parse(trimmed)
  } catch {
    return parseExternalAddress(trimmed)
  }
}

export function isTonAddress(value: string, kind: TonAddressKind): boolean {
  try {
    parseTonAddress(value, kind)
    return true
  } catch {
    return false
  }
}

function parseExternalAddress(value: string): ExternalAddress {
  const match = /^External<(\d+):(\d+)>$/.exec(value)
  if (!match) {
    throw new Error("External address must use External<bits:value> format.")
  }

  const bits = Number(match[1])
  const addressValue = BigInt(match[2])
  if (!Number.isSafeInteger(bits) || bits < 0 || bits > 511) {
    throw new Error("External address bit length must be between 0 and 511.")
  }
  if (addressValue >= 1n << BigInt(bits)) {
    throw new Error("External address value does not fit its bit length.")
  }

  return new ExternalAddress(addressValue, bits)
}

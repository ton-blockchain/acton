import {Address} from "@ton/core"

/** Field context keeps TON meanings out of the generic recursive form structure. */
export interface FieldContext {
  readonly parameter: number
  readonly field: string
  readonly owner?: string
  /** Distinguishes identically named nested limits for bytes, gas and logical time */
  readonly parentField?: string
  readonly mapRole?: "key" | "value"
  readonly mapKey?: string
}

export interface FieldSemantics {
  readonly format?: "address" | "wide-address" | "hash" | "date" | "float32"
  readonly scale?: bigint
  readonly unit?: string
}

const COINS = new Set([
  "mint_new_price",
  "mint_add_price",
  "min_stake",
  "max_stake",
  "min_total_stake",
  "masterchain_block_fee",
  "basechain_block_fee",
  "deposit",
  "default_flat_fine",
  "flat_gas_price",
  "freeze_due_limit",
  "delete_due_limit",
  "lump_price",
  "bridge_burn_fee",
  "bridge_mint_fee",
  "wallet_min_tons_for_storage",
  "wallet_gas_consumption",
  "minter_min_tons_for_storage",
  "discover_gas_consumption",
  "burn_bridge_fee",
])

/** Human units are exact rational conversions; no money or 256-bit value uses Number. */
export function fieldSemantics({
  parameter,
  field,
  owner,
  parentField,
  mapRole,
  mapKey,
}: FieldContext): FieldSemantics {
  if (mapRole === "key") {
    if (parameter === 31 || field === "oracles") return {format: "address"}
    if (parameter === 44) return {format: "wide-address"}
    if (parameter === 39 || parameter === 45) return {format: "hash"}
    return {}
  }
  if (mapRole === "value") {
    if (field === "oracles") return {format: "hash"}
    if (field === "noncritical_params") {
      const id = Number(mapKey)
      if ([2, 5].includes(id)) return {format: "float32", unit: "×"}
      if ([0, 1, 3, 4, 6, 7, 8, 11, 13, 14].includes(id)) return {unit: "ms"}
      if (id === 9) return {unit: "bytes / s"}
    }
    return {}
  }
  if (
    /^(config|elector|minter|fee_collector|dns_root|blackhole)_addr$/.test(field) ||
    ["bridge_address", "oracle_mutlisig_address", "oracles_address", "address"].includes(field)
  ) {
    return {format: "address"}
  }
  if (
    [
      "utime_since",
      "utime_until",
      "valid_until",
      "suspended_until",
      "utime_since",
      "enabled_since",
    ].includes(field)
  )
    return {format: "date"}
  if (COINS.has(field)) return {scale: 1_000_000_000n, unit: "GRAM"}
  if (["bit_price_ps", "cell_price_ps", "mc_bit_price_ps", "mc_cell_price_ps"].includes(field))
    return {scale: 65_536_000_000_000n, unit: "GRAM / s"}
  if (
    field === "gas_price" ||
    (owner?.startsWith("MsgForwardPrices") && ["bit_price", "_cell_price"].includes(field))
  ) {
    return {scale: 65_536_000_000_000n, unit: "GRAM"}
  }
  if (["max_stake_factor", "first_frac", "next_frac"].includes(field))
    return {scale: 65_536n, unit: "×"}
  if (/(_ms|_millis)$/.test(field)) return {unit: "ms"}
  if (
    /(_sec|_seconds|_lifetime|_timeout|_interval)$/.test(field) ||
    [
      "validators_elected_for",
      "elections_start_before",
      "elections_end_before",
      "stake_held_for",
    ].includes(field)
  )
    return {unit: "s"}
  if (
    field.includes("bytes") ||
    field === "max_ext_msg_size" ||
    (owner === "ParamLimits" && ["bytes", "collated_data"].includes(parentField ?? ""))
  )
    return {unit: "bytes"}
  if (field.endsWith("bits")) return {unit: "bits"}
  if (field.endsWith("cells")) return {unit: "cells"}
  return {}
}

export function fieldLabel(field: string): string {
  const labels: Record<string, string> = {
    anon0: "Value",
    mc: "Masterchain",
    shard: "Shardchains",
    _cell_price: "Cell price",
    pubkey: "Public key",
    adnl_addr: "ADNL address",
    oracle_mutlisig_address: "Oracle multisig address",
  }
  if (labels[field]) return labels[field]
  const label = field
    .replace(/^_/, "")
    .replace(/_addr$/, "_address")
    .replaceAll("_", " ")
  return label.charAt(0).toUpperCase() + label.slice(1)
}

export function decimalValue(value: bigint, scale: bigint): string {
  const sign = value < 0n ? "-" : ""
  const magnitude = value < 0n ? -value : value
  let result = `${sign}${magnitude / scale}`
  let rest = magnitude % scale
  if (rest) result += "."
  while (rest) {
    rest *= 10n
    result += String(rest / scale)
    rest %= scale
  }
  return result
}

export function scaledInteger(text: string, scale = 1n): bigint {
  const match = text.trim().match(/^(-?)(\d+)(?:\.(\d+))?$/)
  if (!match) {
    if (scale === 1n && /^0x[\da-f]+$/i.test(text.trim())) return BigInt(text.trim())
    throw new Error("Enter a decimal number")
  }
  const fraction = match[3] ?? ""
  const numerator = BigInt(String(match[2]) + fraction) * scale
  const denominator = 10n ** BigInt(fraction.length)
  if (numerator % denominator)
    throw new Error("This value cannot be represented exactly by the network parameter")
  return ((match[1] ? -1n : 1n) * numerator) / denominator
}

export function parseConfigAddress(text: string, wide = false): bigint {
  const address = Address.parse(text.trim())
  if (!wide && address.workChain !== -1) throw new Error("Use a masterchain address (workchain -1)")
  const hash = BigInt(`0x${address.hash.toString("hex")}`)
  return wide ? (BigInt.asUintN(32, BigInt(address.workChain)) << 256n) | hash : hash
}

export function formatConfigAddress(value: bigint, wide = false): string {
  const workchain = wide ? Number(BigInt.asIntN(32, value >> 256n)) : -1
  return `${workchain}:${BigInt.asUintN(256, value).toString(16).padStart(64, "0")}`
}

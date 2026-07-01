import type {ContractData} from "@acton/shared-ui"
import {Address} from "@ton/core"

export function findAddressContract(
  address: string,
  contracts: Map<string, ContractData> | undefined,
): ContractData | undefined {
  if (!contracts) {
    return undefined
  }

  try {
    const parsed = Address.parse(address)
    return contracts.get(parsed.toString())
  } catch {
    return contracts.get(address)
  }
}

export function isTonAddress(value: string): boolean {
  try {
    Address.parse(value)
    return true
  } catch {
    return false
  }
}

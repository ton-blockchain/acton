import {createHash} from "node:crypto"

import {Address, type Contract} from "@ton/core"

export function addressFromSeed(seed: string, workchain = 0): Address {
  return new Address(workchain, createHash("sha256").update(seed).digest())
}

export function parseAddress(address: Address | string): Address {
  return typeof address === "string" ? Address.parse(address) : address
}

export function formatAddress(address: Address): string {
  return address.toString({bounceable: true, testOnly: false, urlSafe: true})
}

export function isContract(value: Contract | Address | string): value is Contract {
  return typeof value === "object" && "address" in value && Address.isAddress(value.address)
}

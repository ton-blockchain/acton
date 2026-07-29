import addressesJson from "./addresses.json" with {type: "json"}

export interface AddressRegistryEntry {
  readonly address: string
  readonly name: string
}

export const addresses: readonly AddressRegistryEntry[] = addressesJson

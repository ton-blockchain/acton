import mainnetJson from "./mainnet.json" with {type: "json"}
import testnetJson from "./testnet.json" with {type: "json"}

export interface AddressRegistryEntry {
  readonly address: string
  readonly name: string
}

export const getMainnetAddresses = (): readonly AddressRegistryEntry[] => mainnetJson

export const getTestnetAddresses = (): readonly AddressRegistryEntry[] => testnetJson

export const addresses: readonly AddressRegistryEntry[] = [...mainnetJson, ...testnetJson]

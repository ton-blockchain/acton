import jettonsJson from "./jettons.json" with {type: "json"}

/** Search metadata for a mainnet jetton master verified by ton-assets. */
export interface JettonRegistryEntry {
  readonly address: string
  readonly image?: string
  readonly name: string
  readonly symbol: string
}

/** Returns the bundled ton-assets jetton catalog used by Explorer search. */
export const getMainnetJettons = (): readonly JettonRegistryEntry[] => jettonsJson

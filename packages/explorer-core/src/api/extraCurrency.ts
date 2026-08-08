export interface ExtraCurrencyMetadata {
  readonly decimals: number
  readonly origin?: {
    readonly label: string
    readonly linkLabel: string
    readonly source:
      | {readonly kind: "external"; readonly url: string}
      | {readonly kind: "transaction"; readonly hash: string}
  }
  readonly symbol: string
}

const DEFAULT_EXTRA_CURRENCY_DECIMALS = 9

const EXTRA_CURRENCY_METADATA: Readonly<Record<number, ExtraCurrencyMetadata>> = {
  [-17]: {
    decimals: 9,
    origin: {
      label: "Origin: TON zero state",
      linkLabel: "View zero-state source",
      source: {
        kind: "external",
        url: "https://github.com/ton-blockchain/ton/blob/70d73c87ad489f422a09f64a78f4fdc74edcb596/crypto/smartcont/gen-zerostate-test.fif#L203",
      },
    },
    symbol: "UNDEFINED",
  },
  100: {
    decimals: 8,
    origin: {
      label: "Origin: Testnet Extra Currency Minter · 5 Dec 2024",
      linkLabel: "View mint transaction in ActonScan",
      source: {
        kind: "transaction",
        hash: "d73eff828d5bbeb748400cb5112a06a95a08f1e70c267fdc1ce9d23f799ec928",
      },
    },
    symbol: "ECHIDNA",
  },
  239: {
    decimals: 5,
    origin: {
      label: "Origin: TON zero state",
      linkLabel: "View zero-state source",
      source: {
        kind: "external",
        url: "https://github.com/ton-blockchain/ton/blob/70d73c87ad489f422a09f64a78f4fdc74edcb596/crypto/smartcont/gen-zerostate-test.fif#L203",
      },
    },
    symbol: "FMS",
  },
}

/**
 * Extra-currency cells only contain an ID and raw amount, so display metadata
 * must come from an off-chain registry. Unknown currencies use the conventional
 * `$<id>` symbol and nine decimal places while keeping raw units available.
 */
export function getExtraCurrencyMetadata(id: number): ExtraCurrencyMetadata {
  return (
    EXTRA_CURRENCY_METADATA[id] ?? {
      decimals: DEFAULT_EXTRA_CURRENCY_DECIMALS,
      symbol: `$${id}`,
    }
  )
}

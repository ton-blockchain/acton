import type {AccountStateTokenInfo} from "../api/types"

/** A contract type that the explorer can identify from a known code hash. */
export type AccountCodeHashContractType = "locker" | "vesting" | "wallet_v5r1"

const ACCOUNT_CONTRACT_TYPES_BY_CODE_HASH: Readonly<Record<string, AccountCodeHashContractType>> = {
  "830c99a447d0974db6d86ed7d89fe4f2d2ec22358cb64287e2d31e563ebde547": "locker",
  "6a05d82d45c933dd2ddf7b338db7f6fbce6d5e2ec3b142e5fc53a94395dca252": "locker",
  b48b531abec3b714638291f7d77ed6dc9f6a2729efca20477137374d4ae8b590: "vesting",
  "20834b7b72b112147e1b2fb457b84e74d1a30f04f737d4f62a668e9552d2b72f": "wallet_v5r1",
  "6398c607b11b539775a583f7bacb80c1d22087100c4f40df5fb502d2b726aed2": "wallet_v5r1",
}

const ACCOUNT_CONTRACT_HINTS = {
  jetton_master: {
    interfaces: ["jetton_master"],
    tokenInfoType: "jetton_masters",
  },
  jetton_wallet: {
    interfaces: ["jetton_wallet"],
    tokenInfoType: "jetton_wallets",
  },
  nft_collection: {
    interfaces: ["nft_collection"],
    tokenInfoType: "nft_collections",
  },
  nft_item: {
    interfaces: ["nft_item", "nft_item_simple"],
    tokenInfoType: "nft_items",
  },
} as const

type AccountContractType = keyof typeof ACCOUNT_CONTRACT_HINTS

/**
 * Returns the known contract type for a canonical code hash.
 *
 * The hash must be a lowercase hexadecimal string without a `0x` prefix.
 * Unknown or missing hashes return `undefined`.
 */
export function getAccountContractTypeByCodeHash(
  codeHash: string | undefined,
): AccountCodeHashContractType | undefined {
  return codeHash === undefined ? undefined : ACCOUNT_CONTRACT_TYPES_BY_CODE_HASH[codeHash]
}

export function hasAccountInterface(interfaces: readonly string[], expected: string): boolean {
  return interfaces.some(iface => iface.trim().toLowerCase() === expected)
}

export function hasTokenInfoType(
  tokenInfo: readonly AccountStateTokenInfo[],
  expected: string,
): boolean {
  return tokenInfo.some(info => info.type === expected)
}

export function hasAccountContractHint(
  interfaces: readonly string[],
  tokenInfo: readonly AccountStateTokenInfo[],
  expected: AccountContractType,
): boolean {
  const hint = ACCOUNT_CONTRACT_HINTS[expected]
  const hasInterface = hint.interfaces.some(expectedInterface =>
    hasAccountInterface(interfaces, expectedInterface),
  )

  return hasInterface || hasTokenInfoType(tokenInfo, hint.tokenInfoType)
}

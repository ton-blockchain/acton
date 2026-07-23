export type WalletVersion = "v4r2" | "v5r1"
export type WalletNetworkId = "mainnet" | "testnet"

export interface WalletNetwork {
  readonly id: WalletNetworkId
  readonly name: string
  readonly chainId: string
  readonly endpoint: string
  readonly explorerUrl: string
}

export interface WalletRecord {
  readonly id: string
  readonly name: string
  readonly address: string
  readonly publicKey: string
  readonly version: WalletVersion
  readonly network: WalletNetworkId
  readonly createdAt: string
}

export const WALLET_NETWORKS: Readonly<Record<WalletNetworkId, WalletNetwork>> = {
  mainnet: {
    id: "mainnet",
    name: "Mainnet",
    chainId: "-239",
    endpoint: "https://toncenter.com",
    explorerUrl: "https://actonscan.com",
  },
  testnet: {
    id: "testnet",
    name: "Testnet",
    chainId: "-3",
    endpoint: "https://testnet.toncenter.com",
    explorerUrl: "https://actonscan.com",
  },
}

export function shortenAddress(address: string, edgeLength = 7): string {
  if (address.length <= edgeLength * 2 + 1) {
    return address
  }
  return `${address.slice(0, edgeLength)}…${address.slice(-edgeLength)}`
}

export function getAccountExplorerUrl(wallet: WalletRecord): string {
  const network = WALLET_NETWORKS[wallet.network]
  const testnetQuery = wallet.network === "testnet" ? "?testnet=true" : ""
  return `${network.explorerUrl}/address/${encodeURIComponent(wallet.address)}${testnetQuery}`
}

export function getTransactionExplorerUrl(wallet: WalletRecord, hash: string): string {
  const network = WALLET_NETWORKS[wallet.network]
  const testnetQuery = wallet.network === "testnet" ? "?testnet=true" : ""
  return `${network.explorerUrl}/tx/${encodeURIComponent(hash)}${testnetQuery}`
}

export function formatTonBalance(nano: string | undefined): string {
  if (nano === undefined) {
    return "—"
  }

  const value = BigInt(nano)
  const isNegative = value < 0n
  const absolute = isNegative ? -value : value
  const whole = absolute / 1_000_000_000n
  const fraction = (absolute % 1_000_000_000n)
    .toString()
    .padStart(9, "0")
    .replace(/0+$/, "")
    .slice(0, 4)

  return `${isNegative ? "−" : ""}${whole.toLocaleString("en-US")}${fraction ? `.${fraction}` : ""}`
}
